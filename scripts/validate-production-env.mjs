const required = ["POSTGRES_PASSWORD", "DECISION_KEY_ID", "DECISION_SIGNING_KEY_B64"];
const missing = required.filter((name) => !process.env[name]?.trim());
if (missing.length) {
  console.error(`Missing required production environment variables: ${missing.join(", ")}`);
  process.exit(1);
}

const signingKey = process.env.DECISION_SIGNING_KEY_B64.trim();
if (!/^[A-Za-z0-9+/]+={0,2}$/.test(signingKey) || signingKey.length < 43) {
  console.error("DECISION_SIGNING_KEY_B64 is not a valid-looking base64 key");
  process.exit(1);
}

if (process.env.APP_ENV !== "production") console.warn("Warning: APP_ENV is not production");
console.log("Production environment preflight passed; secrets were not printed.");
