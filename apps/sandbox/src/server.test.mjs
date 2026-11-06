import test from "node:test";
import assert from "node:assert/strict";
import { createSandboxServer } from "./server.mjs";

let server;
test.before(async () => { server = createSandboxServer({ port: 18090, timeoutMs: 10 }); await new Promise((resolve) => server.once("listening", resolve)); });
test.after(() => server.close());
test("returns a 402 quote without payment", async () => { const response = await fetch("http://127.0.0.1:18090/resource?scenario=success"); assert.equal(response.status, 402); assert.equal((await response.json()).accepts[0].amount_atomic, "10"); });
test("simulates success, rejection, and mismatch", async () => { for (const [scenario, expected] of [["success", "settled"], ["rejection", "rejected"], ["mismatch", "mismatch"]]) { const response = await fetch(`http://127.0.0.1:18090/resource?scenario=${scenario}`, { headers: { "x-payment": "sandbox-proof" } }); const body = await response.json(); assert.equal(body.settlement?.outcome ?? body.status, expected); } });
