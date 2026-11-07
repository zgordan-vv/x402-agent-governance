import { createServer } from "node:http";

export const scenarios = new Set(["success", "rejection", "mismatch", "timeout"]);
export function createSandboxServer({ port = Number(process.env.PORT ?? 8090), timeoutMs = 5000 } = {}) {
  return createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", `http://${request.headers.host ?? "localhost"}`);
    const scenario = url.searchParams.get("scenario") ?? "success";
    if (!scenarios.has(scenario)) return json(response, 400, { error: "unknown scenario", scenarios: [...scenarios] });
    if (scenario === "timeout") await new Promise((resolve) => setTimeout(resolve, timeoutMs));
    if (url.pathname === "/demo") return html(response, demoHtml);
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
function html(response, value) { response.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" }); response.end(value); }

const demoHtml = `<!doctype html><meta charset="utf-8"><title>x402 sandbox demo</title>
<style>body{font:16px system-ui;max-width:760px;margin:40px auto;padding:0 20px;background:#101522;color:#edf2f7}button{margin:4px;padding:10px 14px;border:1px solid #60708c;border-radius:6px;background:#1e2a42;color:inherit;cursor:pointer}button:hover{background:#2d3e5d}table{width:100%;margin-top:20px;border-collapse:collapse}td,th{padding:10px;text-align:left;border-bottom:1px solid #33415c}.muted{color:#a9b7ce}</style>
<h1>x402 sandbox demo</h1><p class="muted">Deterministic, no-money scenarios. This does not write to production.</p>
<p>${[...scenarios].map((name) => `<button data-scenario="${name}">${name}</button>`).join("")}</p>
<table><thead><tr><th>Scenario</th><th>HTTP</th><th>Outcome</th><th>Latency</th></tr></thead><tbody id="results"><tr><td colspan="4" class="muted">Choose a scenario.</td></tr></tbody></table>
<script>const results=document.querySelector("#results");document.querySelectorAll("button").forEach(button=>button.onclick=async()=>{const scenario=button.dataset.scenario;const started=performance.now();try{const response=await fetch("/resource?scenario="+encodeURIComponent(scenario),{headers:{"x-payment":"sandbox-proof"}});const body=await response.json();const outcome=body.settlement?.outcome??body.status??body.outcome;results.innerHTML='<tr><td>'+scenario+'</td><td>'+response.status+'</td><td>'+outcome+'</td><td>'+Math.round(performance.now()-started)+' ms</td></tr>';}catch(error){results.innerHTML='<tr><td>'+scenario+'</td><td>error</td><td>unknown</td><td>'+Math.round(performance.now()-started)+' ms</td></tr>';}});</script>`;
if (import.meta.url === `file://${process.argv[1]}`) createSandboxServer();
