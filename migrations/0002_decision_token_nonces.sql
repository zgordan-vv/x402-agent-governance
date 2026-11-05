CREATE TABLE payments.decision_token_nonces (
    nonce uuid PRIMARY KEY,
    decision_id uuid NOT NULL REFERENCES payments.decisions(decision_id),
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX decision_token_nonces_expiry_idx
    ON payments.decision_token_nonces (expires_at);
