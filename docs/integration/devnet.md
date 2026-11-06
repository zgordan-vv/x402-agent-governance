# Solana devnet integration

This is an opt-in compatibility check for a disposable Solana devnet wallet. It does not custody keys, sign transactions, or send funds. The application receives only a public wallet address and/or a transaction signature; signing remains in the customer-owned wallet or SDK.

## Preflight

```bash
export SOLANA_RPC_URL=https://api.devnet.solana.com
export DEVNET_WALLET_ADDRESS=<disposable-public-address>
pnpm devnet:preflight
```

The command checks RPC health and the wallet balance. It never asks for a seed phrase or private key. Use a dedicated devnet wallet and keep the balance small.

## Verify a transaction

After sending a tiny devnet transaction from the wallet, verify its evidence without exposing credentials:

```bash
export DEVNET_SIGNATURE=<signature>
pnpm devnet:preflight
```

The output reports whether the signature is visible, the slot, execution error (if any), and a devnet Explorer URL. A `null` transaction is `unknown`, not success; wait for confirmation and retry.

## Promotion scenarios

Run the normal SDK/API flow with the same disposable wallet and record three separate cases:

1. an allowed intent followed by a matching signature;
2. a denied intent where the signer is not called;
3. a mismatch (wrong amount or recipient) that is recorded as reconciliation evidence, never as success.

Revoke the temporary dashboard/agent credential and discard the wallet after the check. Never commit RPC tokens, bearer credentials, seed phrases, or private keys.
