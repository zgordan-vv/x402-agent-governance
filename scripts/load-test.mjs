const url = process.env.LOAD_URL ?? "http://127.0.0.1:8090/health";
const total = Number(process.env.LOAD_REQUESTS ?? 200);
const concurrency = Number(process.env.LOAD_CONCURRENCY ?? 20);
const samples = [];
let next = 0;
async function worker() { while (true) { const index = next++; if (index >= total) return; const start = performance.now(); try { const response = await fetch(url); await response.arrayBuffer(); samples.push({ ms: performance.now() - start, ok: response.ok, status: response.status }); } catch { samples.push({ ms: performance.now() - start, ok: false, status: 0 }); } } }
await Promise.all(Array.from({ length: Math.min(concurrency, total) }, worker));
const sorted = samples.map((sample) => sample.ms).sort((a, b) => a - b);
const percentile = (p) => sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)];
const failures = samples.filter((sample) => !sample.ok).length;
console.log(JSON.stringify({ url, requests: total, concurrency, failures, p50_ms: Number(percentile(0.5).toFixed(2)), p95_ms: Number(percentile(0.95).toFixed(2)), p99_ms: Number(percentile(0.99).toFixed(2)) }, null, 2));
if (failures > 0) process.exitCode = 1;
