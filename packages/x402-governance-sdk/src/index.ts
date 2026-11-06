export type DecisionOutcome = "allow" | "deny" | "approval_required";
export type Decision = { outcome: DecisionOutcome; decision_id: string; intent_id: string; reason_code: string; expires_at: string; decision_token?: string };
export type PaymentRequirements = { network: string; asset: string; amount_atomic: string; pay_to: string; merchant_origin?: string; merchant_path_prefix?: string; quote_expires_at?: string; [key: string]: unknown };
export type SignedPayment = { header: string; evidence?: Record<string, unknown> };
export type Signer = (payment: PaymentRequirements & { decisionToken: string }) => Promise<SignedPayment>;
export type FetchLike = typeof fetch;
export type GuardrailClientOptions = { baseUrl: string; credential: string; agentId: string; fetch?: FetchLike; maxEvaluationRetries?: number };

export class GovernanceError extends Error { readonly code: string; readonly details?: unknown; constructor(message: string, code: string, details?: unknown) { super(message); this.name = "GovernanceError"; this.code = code; this.details = details; } }
export class ApprovalRequiredError extends GovernanceError { readonly decision: Decision; constructor(decision: Decision) { super("Payment requires approval", "approval_required", decision); this.decision = decision; } }
export class PaymentDeniedError extends GovernanceError { readonly decision: Decision; constructor(decision: Decision) { super("Payment denied by policy", decision.reason_code, decision); this.decision = decision; } }
export class AmbiguousSignerError extends GovernanceError { constructor(cause: unknown) { super("Signer result is ambiguous; do not retry automatically", "signer_ambiguous", redact(cause)); } }

export function parsePaymentRequirements(response: Response, body: unknown): PaymentRequirements {
  if (!body || typeof body !== "object") throw new GovernanceError("Malformed x402 response", "malformed_402");
  const value = body as Record<string, unknown>; const candidate = Array.isArray(value.accepts) ? value.accepts[0] : value.paymentRequirements ?? value;
  if (!candidate || typeof candidate !== "object") throw new GovernanceError("Malformed x402 payment requirements", "malformed_402");
  const raw = candidate as Record<string, unknown>; const network = String(raw.network ?? raw.network_id ?? "").trim(); const asset = String(raw.asset ?? raw.asset_address ?? "").trim(); const amount = String(raw.amount_atomic ?? raw.maxAmountRequired ?? raw.amount ?? "").trim(); const payTo = String(raw.pay_to ?? raw.payTo ?? raw.payToAddress ?? "").trim();
  if (!network || !asset || !/^\d+$/.test(amount) || !payTo) throw new GovernanceError("Malformed x402 payment requirements", "malformed_402");
  return { ...raw, network, asset, amount_atomic: amount, pay_to: payTo, merchant_origin: String(raw.merchant_origin ?? new URL(response.url).origin), merchant_path_prefix: typeof raw.merchant_path_prefix === "string" ? raw.merchant_path_prefix : undefined, quote_expires_at: typeof raw.quote_expires_at === "string" ? raw.quote_expires_at : undefined };
}

export class GuardrailClient {
  private readonly request: FetchLike;
  readonly options: GuardrailClientOptions;
  constructor(options: GuardrailClientOptions) { this.options = options; this.request = options.fetch ?? fetch; }
  async evaluate(requirements: PaymentRequirements, requestHash: string, quoteExpiresAt: string): Promise<Decision> {
    const payload = { idempotency_key: cryptoRandomId(), agent_id: this.options.agentId, network: requirements.network, asset: requirements.asset, amount_atomic: requirements.amount_atomic, merchant_origin: requirements.merchant_origin, merchant_path_prefix: requirements.merchant_path_prefix ?? null, pay_to: requirements.pay_to, request_hash: requestHash, quote_expires_at: quoteExpiresAt };
    const attempts = this.options.maxEvaluationRetries ?? 1; let last: unknown;
    for (let attempt = 0; attempt <= attempts; attempt++) { try { const response = await this.request(`${this.options.baseUrl.replace(/\/$/, "")}/v1/evaluate`, { method: "POST", headers: { authorization: `Bearer ${this.options.credential}`, "content-type": "application/json" }, body: JSON.stringify(payload) }); const body = await readJson(response); if (!response.ok) throw new GovernanceError("Governance evaluation failed", String((body as any)?.error?.code ?? "evaluation_failed"), redact(body)); return body as Decision; } catch (error) { last = error; if (attempt === attempts) throw error; } }
    throw last;
  }
  async settle(decision: Decision, evidence: SignedPayment, status: "settled" | "failed" | "unknown", observed?: Pick<PaymentRequirements, "amount_atomic" | "pay_to">): Promise<Record<string, unknown>> {
    const bodyPayload = JSON.stringify({ idempotency_key: `settlement:${decision.decision_id}`, decision_id: decision.decision_id, status, transaction_signature: typeof evidence.evidence?.transaction_signature === "string" ? evidence.evidence.transaction_signature : null, evidence: redact(evidence.evidence ?? {}), observed_amount_atomic: observed?.amount_atomic ?? null, observed_pay_to: observed?.pay_to ?? null });
    let lastBody: unknown;
    for (let attempt = 0; attempt < 2; attempt++) {
      const response = await this.request(`${this.options.baseUrl.replace(/\/$/, "")}/v1/settlements`, { method: "POST", headers: { authorization: `Bearer ${this.options.credential}`, "content-type": "application/json" }, body: bodyPayload });
      const body = await readJson(response); if (response.ok) return body as Record<string, unknown>; lastBody = body;
    }
    throw new GovernanceError("Settlement reporting failed", String((lastBody as any)?.error?.code ?? "settlement_failed"), redact(lastBody));
  }
  async governedFetch(input: RequestInfo | URL, init: RequestInit & { signer: Signer; requestHash?: string }): Promise<Response> {
    const { signer, requestHash, ...fetchInit } = init; const initial = await this.request(input, fetchInit); if (initial.status !== 402) return initial; const requirements = parsePaymentRequirements(initial, await readJson(initial)); const quoteExpiry = requirements.quote_expires_at ?? new Date(Date.now() + 30_000).toISOString(); const decision = await this.evaluate(requirements, requestHash ?? await hashRequest(input, fetchInit), quoteExpiry);
    if (decision.outcome === "deny") throw new PaymentDeniedError(decision); if (decision.outcome === "approval_required") throw new ApprovalRequiredError(decision); if (!decision.decision_token) throw new GovernanceError("Allow decision has no signed token", "missing_decision_token");
    let signed: SignedPayment; try { signed = await signer({ ...requirements, decisionToken: decision.decision_token }); } catch (error) { throw new AmbiguousSignerError(error); }
    const headers = new Headers(fetchInit.headers); headers.set("x-payment", signed.header); const paid = await this.request(input, { ...fetchInit, headers }); if (paid.ok) await this.settle(decision, signed, "settled", requirements); return paid;
  }
}

async function readJson(response: Response): Promise<unknown> { const text = await response.text(); if (!text) return {}; try { return JSON.parse(text); } catch { throw new GovernanceError("Response is not valid JSON", "invalid_json"); } }
async function hashRequest(input: RequestInfo | URL, init: RequestInit): Promise<string> { const text = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url; const canonical = JSON.stringify({ method: init.method ?? "GET", url: text, body: typeof init.body === "string" ? init.body : null }); const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(canonical)); return `sha256:${Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("")}`; }
function cryptoRandomId(): string { const bytes = new Uint8Array(16); crypto.getRandomValues(bytes); return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(""); }
export function redact(value: unknown): unknown { if (Array.isArray(value)) return value.map(redact); if (!value || typeof value !== "object") return value; const secret = /token|credential|authorization|private|secret|signature|payment/i; return Object.fromEntries(Object.entries(value as Record<string, unknown>).map(([key, item]) => [key, secret.test(key) ? "[REDACTED]" : redact(item)])); }
