import { createServer } from "node:http";

export const scenarios = new Set(["success", "rejection", "mismatch", "timeout"]);
export function createSandboxServer({ port = Number(process.env.PORT ?? 8090), timeoutMs = 5000 } = {}) {
  return createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", `http://${request.headers.host ?? "localhost"}`);
    const scenario = url.searchParams.get("scenario") ?? "success";
    if (!scenarios.has(scenario)) return json(response, 400, { error: "unknown scenario", scenarios: [...scenarios] });
    if (scenario === "timeout") await new Promise((resolve) => setTimeout(resolve, timeoutMs));
    if (url.pathname === "/health") return json(response, 200, { service: "sandbox-merchant", ready: true });
    if (url.pathname !== "/resource") return json(response, 404, { error: "not_found" });
    const requirements = { network: "sandbox", asset: "USDC", amount_atomic: "10", pay_to: "sandbox-merchant", merchant_origin: `http://${request.headers.host ?? "localhost"}`, quote_expires_at: new Date(Date.now() + 30_000).toISOString() };
    if (!request.headers["x-payment"]) return json(response, 402, { accepts: [requirements], resource: "/resource", description: "Sandbox resource; no real funds" });
    if (scenario === "rejection") return json(response, 402, { error: "simulated facilitator rejection", status: "rejected" });
    if (scenario === "mismatch") return json(response, 200, { status: "accepted", settlement: { outcome: "mismatch", observed_amount_atomic: "11", expected_amount_atomic: "10" } });
    return json(response, 200, { status: "accepted", settlement: { outcome: "settled", transaction_signature: "sandbox-signature" } });
  }).listen(port);
}
function json(response, status, value) { response.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" }); response.end(JSON.stringify(value)); }
if (import.meta.url === `file://${process.argv[1]}`) createSandboxServer();
