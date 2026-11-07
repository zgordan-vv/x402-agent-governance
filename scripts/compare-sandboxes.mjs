const environments = [
  ["sandbox", process.env.SANDBOX_URL ?? "http://127.0.0.1:8090"],
  ["staging", process.env.STAGING_SANDBOX_URL ?? "http://127.0.0.1:8091"],
];
const scenarios = ["success", "rejection", "mismatch"];
const rows = [];

for (const scenario of scenarios) {
  const outcomes = [];
  for (const [environment, baseUrl] of environments) {
    const response = await fetch(`${baseUrl}/resource?scenario=${scenario}`, { headers: { "x-payment": "sandbox-proof" } });
    const body = await response.json();
    outcomes.push([environment, response.status, body.settlement?.outcome ?? body.status ?? body.outcome]);
  }
  rows.push({ scenario, sandbox: outcomes[0][2], staging: outcomes[1][2], same: outcomes[0][2] === outcomes[1][2] });
}

console.table(rows);
if (rows.some((row) => !row.same)) throw new Error("sandbox environments produced different outcomes");
console.log("Environment comparison passed: policy outcomes are equivalent.");
