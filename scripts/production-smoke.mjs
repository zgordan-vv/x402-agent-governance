const baseUrl = (process.env.GOVERNANCE_URL ?? "http://127.0.0.1:8080").replace(/\/$/, "");
const paths = ["/health/live", "/health/ready", "/metrics", "/openapi.json"];
const failures = [];
for (const path of paths) {
  try {
    const response = await fetch(`${baseUrl}${path}`, { signal: AbortSignal.timeout(5_000) });
    const body = await response.text();
    if (!response.ok) failures.push(`${path}: HTTP ${response.status}`);
    else if (path === "/metrics" && !body.includes("governance_api_up 1")) failures.push(`${path}: health metric missing`);
    else if (path === "/openapi.json" && !body.includes("/v1/evaluate")) failures.push(`${path}: evaluate route missing`);
    else console.log(`${path}: OK (${response.status})`);
  } catch (error) {
    failures.push(`${path}: ${error instanceof Error ? error.message : String(error)}`);
  }
}
if (failures.length) { console.error(failures.join("\n")); process.exit(1); }
console.log(`Production smoke test passed for ${baseUrl}`);
