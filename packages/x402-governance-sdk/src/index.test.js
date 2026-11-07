import test from "node:test";
import assert from "node:assert/strict";
import { ApprovalRequiredError, GovernanceError, GuardrailClient, PaymentDeniedError, parsePaymentRequirements, redact } from "./index.ts";

function response(body, status = 402, url = "https://merchant.test/pay") { const result = new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } }); Object.defineProperty(result, "url", { value: url }); return result; }
test("parser accepts x402 accepts shape", () => { const parsed = parsePaymentRequirements(response({ accepts: [{ network: "solana-devnet", asset: "USDC", amount: "10", payTo: "merchant" }] }), { accepts: [{ network: "solana-devnet", asset: "USDC", amount: "10", payTo: "merchant" }] }); assert.equal(parsed.amount_atomic, "10"); assert.equal(parsed.pay_to, "merchant"); });
test("deny never invokes signer", async () => { let signerCalls = 0; let calls = 0; const client = new GuardrailClient({ baseUrl: "https://guardrail.test", credential: "secret", agentId: "agent", fetch: async (_input, init) => { calls++; return response(calls === 1 ? { accepts: [{ network: "solana-devnet", asset: "USDC", amount: "10", payTo: "merchant" }] } : { outcome: "deny", decision_id: "d", intent_id: "i", reason_code: "merchant_not_allowlisted", expires_at: new Date().toISOString() }, calls === 1 ? 402 : 200); } }); await assert.rejects(() => client.governedFetch("https://merchant.test/pay", { signer: async () => { signerCalls++; return { header: "never" }; } }), PaymentDeniedError); assert.equal(signerCalls, 0); });
test("evaluation retry reuses idempotency key", async () => { const bodies = []; let call = 0; const client = new GuardrailClient({ baseUrl: "https://guardrail.test", credential: "secret", agentId: "agent", maxEvaluationRetries: 1, fetch: async (_input, init) => { call++; if (init?.body) bodies.push(JSON.parse(String(init.body))); return call === 1 ? response({ error: { code: "temporary" } }, 503) : response({ outcome: "allow", decision_id: "d", intent_id: "i", reason_code: "within_budget_and_allowlist", expires_at: new Date(Date.now() + 30000).toISOString(), decision_token: "token" }, 200); } }); const result = await client.evaluate({ network: "solana-devnet", asset: "USDC", amount_atomic: "10", pay_to: "merchant" }, "sha256:test", new Date(Date.now() + 30000).toISOString()); assert.equal(result.outcome, "allow"); assert.equal(bodies[0].idempotency_key, bodies[1].idempotency_key); });
test("signer failure is ambiguous and secrets are redacted", async () => { const client = new GuardrailClient({ baseUrl: "https://guardrail.test", credential: "secret", agentId: "agent", fetch: async () => response({ accepts: [{ network: "solana-devnet", asset: "USDC", amount: "10", payTo: "merchant" }] }) }); await assert.rejects(() => client.governedFetch("https://merchant.test/pay", { signer: async () => { throw new Error("private key secret"); } }), GovernanceError); assert.deepEqual(redact({ authorization: "secret", nested: { value: "ok" } }), { authorization: "[REDACTED]", nested: { value: "ok" } }); assert.equal(new ApprovalRequiredError({ outcome: "approval_required", decision_id: "d", intent_id: "i", reason_code: "approval_required", expires_at: "" }).code, "approval_required"); });
test("governedFetch evaluates before signing and reports settlement", async () => {
  const calls = [];
  const client = new GuardrailClient({ baseUrl: "https://guardrail.test", credential: "secret", agentId: "agent", fetch: async (input, init) => {
    const url = String(input); calls.push({ url, init });
    if (url === "https://merchant.test/pay" && calls.length === 1) return response({ accepts: [{ network: "solana-devnet", asset: "USDC", amount: "10", payTo: "merchant", quote_expires_at: new Date(Date.now() + 30_000).toISOString() }] });
    if (url.endsWith("/v1/evaluate")) return response({ outcome: "allow", decision_id: "decision-1", intent_id: "intent-1", reason_code: "within_budget_and_allowlist", expires_at: new Date(Date.now() + 30_000).toISOString(), decision_token: "decision-token" }, 200);
    if (url === "https://merchant.test/pay") return response({ status: "accepted" }, 200);
    if (url.endsWith("/v1/settlements")) return response({ outcome: "settled" }, 200);
    throw new Error(`unexpected URL ${url}`);
  } });
  const result = await client.governedFetch("https://merchant.test/pay", { signer: async ({ decisionToken }) => ({ header: `signed:${decisionToken}`, evidence: { transaction_signature: "sandbox-signature" } }) });
  assert.equal(result.status, 200);
  assert.deepEqual(calls.map(({ url }) => url), ["https://merchant.test/pay", "https://guardrail.test/v1/evaluate", "https://merchant.test/pay", "https://guardrail.test/v1/settlements"]);
  assert.equal(calls[2].init.headers.get("x-payment"), "signed:decision-token");
  assert.equal(JSON.parse(calls[3].init.body).status, "settled");
});
