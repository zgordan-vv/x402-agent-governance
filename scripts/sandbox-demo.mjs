import { createSandboxServer, scenarios } from "../apps/sandbox/src/server.mjs";

const port = Number(process.env.SANDBOX_PORT ?? 18090);
const server = createSandboxServer({ port, timeoutMs: 200 });
const baseUrl = `http://127.0.0.1:${port}`;
const results = [];

try {
  for (const scenario of scenarios) {
    const started = Date.now();
    const response = await fetch(`${baseUrl}/resource?scenario=${scenario}`, {
      headers: { "x-payment": "sandbox-proof" },
      signal: AbortSignal.timeout(scenario === "timeout" ? 50 : 1_000),
    }).catch((error) => ({
      status: 599,
      json: async () => ({ outcome: "unknown", error: error.name }),
    }));
    const body = await response.json();
    results.push({ scenario, http_status: response.status, outcome: body.settlement?.outcome ?? body.status ?? body.outcome, elapsed_ms: Date.now() - started });
  }
} finally {
  server.close();
}

console.table(results);
const expected = new Map([
  ["success", "settled"],
  ["rejection", "rejected"],
  ["mismatch", "mismatch"],
  ["timeout", "unknown"],
]);
for (const result of results) {
  if (result.outcome !== expected.get(result.scenario)) {
    throw new Error(`unexpected ${result.scenario} outcome: ${result.outcome}`);
  }
}
console.log("Sandbox demo completed: no real funds were used.");
