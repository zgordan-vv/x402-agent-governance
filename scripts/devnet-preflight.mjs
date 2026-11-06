const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com";
const wallet = process.env.DEVNET_WALLET_ADDRESS;
const signature = process.env.DEVNET_SIGNATURE;

if (!wallet && !signature) {
  console.error("Set DEVNET_WALLET_ADDRESS or DEVNET_SIGNATURE. This script never accepts private keys.");
  process.exit(2);
}

async function rpc(method, params = []) {
  const response = await fetch(rpcUrl, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
  });
  if (!response.ok) throw new Error(`RPC HTTP ${response.status}`);
  const body = await response.json();
  if (body.error) throw new Error(`${method}: ${body.error.message}`);
  return body.result;
}

const health = await rpc("getHealth");
const output = { rpc: rpcUrl, health, network: "solana-devnet" };
if (wallet) {
  const balance = await rpc("getBalance", [wallet, { commitment: "confirmed" }]);
  output.wallet = { address: wallet, lamports: balance.value, sol: balance.value / 1_000_000_000 };
}
if (signature) {
  const transaction = await rpc("getTransaction", [signature, { commitment: "confirmed", maxSupportedTransactionVersion: 0 }]);
  output.transaction = { signature, found: transaction !== null, slot: transaction?.slot ?? null, err: transaction?.meta?.err ?? null };
  output.explorer = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
}
console.log(JSON.stringify(output, null, 2));
