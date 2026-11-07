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

const demoHtml = `<!doctype html><meta charset="utf-8"><title>x402 governance demo</title>
<style>body{font:16px system-ui;max-width:900px;margin:40px auto;padding:0 20px;background:#101522;color:#edf2f7}h1{margin-bottom:8px}h2{margin-top:34px}.muted{color:#a9b7ce}.banner{padding:14px 16px;border:1px solid #45618a;border-radius:8px;background:#18243a}.steps{display:grid;grid-template-columns:repeat(4,1fr);gap:10px}.step,.scenario{padding:14px;border:1px solid #33415c;border-radius:8px;background:#151f32}.step b{display:block;color:#76e1c1;margin-bottom:6px}button{margin:4px 4px 4px 0;padding:10px 14px;border:1px solid #60708c;border-radius:6px;background:#1e2a42;color:inherit;cursor:pointer}button:hover{background:#2d3e5d}button.primary{background:#4fd1b0;color:#08131a;border-color:#4fd1b0;font-weight:700}table{width:100%;margin-top:20px;border-collapse:collapse}td,th{padding:10px;text-align:left;border-bottom:1px solid #33415c}.result{margin-top:16px;min-height:42px}.outcome{font-size:20px;font-weight:700;color:#76e1c1}</style>
<h1>Interactive x402 payment governance</h1><p class="muted">See what happens before an autonomous agent is allowed to pay.</p>
<p class="banner"><b>Safe sandbox:</b> these examples use a local fake merchant. No wallet, private key, blockchain transaction, or production audit record is involved.</p>
<h2>How the lifecycle works</h2><div class="steps"><div class="step"><b>1. Intent</b>Agent receives a quote and proposes amount, asset, network, and recipient.</div><div class="step"><b>2. Policy</b>Governance checks limits, allowlists, approvals, and workspace state.</div><div class="step"><b>3. Signing</b>Only an allowed decision may reach the customer-owned wallet.</div><div class="step"><b>4. Evidence</b>Settlement is recorded as settled, failed, unknown, or mismatch.</div></div>
<h2>Run a scenario</h2><p class="muted">Each result demonstrates a different operational decision.</p><button class="primary" id="run-all">Run all scenarios</button><div id="buttons">${[...scenarios].map((name) => `<button data-scenario="${name}">${name}</button>`).join("")}</div>
<div class="result" id="explanation"></div><table><thead><tr><th>Scenario</th><th>HTTP</th><th>Policy/settlement outcome</th><th>Latency</th></tr></thead><tbody id="results"><tr><td colspan="4" class="muted">Run a scenario to inspect the evidence.</td></tr></tbody></table>
<h2>What this demonstrates</h2><p class="muted">An <b>allow</b> decision is not the same as a confirmed blockchain payment. A rejection stops payment, a mismatch preserves conflicting evidence, and an unknown result stays unresolved instead of being converted into success.</p>
<script>const results=document.querySelector("#results"),explanation=document.querySelector("#explanation");const why={success:"The merchant accepted the sandbox payment and returned settlement evidence.",rejection:"The facilitator rejected the payment; the agent must not treat this as success.",mismatch:"The observed amount differs from the quote, so the evidence is retained as mismatch.",timeout:"The external confirmation did not arrive before the deadline, so the result remains unknown."};async function run(scenario){const started=performance.now();try{const response=await fetch("/resource?scenario="+encodeURIComponent(scenario),{headers:{"x-payment":"sandbox-proof"}});const body=await response.json();const outcome=body.settlement?.outcome??body.status??body.outcome;results.innerHTML='<tr><td>'+scenario+'</td><td>'+response.status+'</td><td class="outcome">'+outcome+'</td><td>'+Math.round(performance.now()-started)+' ms</td></tr>'+results.innerHTML;explanation.innerHTML='<b>'+scenario+'</b>: '+why[scenario];}catch(error){results.innerHTML='<tr><td>'+scenario+'</td><td>error</td><td class="outcome">unknown</td><td>'+Math.round(performance.now()-started)+' ms</td></tr>'+results.innerHTML;}}document.querySelectorAll("[data-scenario]").forEach(button=>button.onclick=()=>run(button.dataset.scenario));document.querySelector("#run-all").onclick=async()=>{results.innerHTML="";for(const scenario of ["success","rejection","mismatch","timeout"])await run(scenario);};</script>`;
if (import.meta.url === `file://${process.argv[1]}`) createSandboxServer();
