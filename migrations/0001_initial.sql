CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE SCHEMA IF NOT EXISTS control;
CREATE SCHEMA IF NOT EXISTS policy;
CREATE SCHEMA IF NOT EXISTS payments;
CREATE SCHEMA IF NOT EXISTS audit;
CREATE SCHEMA IF NOT EXISTS jobs;

CREATE TABLE jobs.work_items (
    job_id uuid PRIMARY KEY,
    job_type text NOT NULL,
    payload jsonb NOT NULL DEFAULT '{}',
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'leased', 'complete', 'retryable', 'dead_letter')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at timestamptz NOT NULL DEFAULT now(),
    lease_until timestamptz,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX jobs_available_idx ON jobs.work_items (status, available_at);

CREATE TABLE control.workspaces (
    workspace_id uuid PRIMARY KEY,
    name text NOT NULL,
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'frozen', 'deleted')),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE control.environments (
    environment_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    name text NOT NULL,
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'frozen', 'deleted')),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, name),
    UNIQUE (workspace_id, environment_id)
);

CREATE TABLE control.agents (
    agent_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    environment_id uuid NOT NULL REFERENCES control.environments(environment_id),
    name text NOT NULL,
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'frozen', 'revoked')),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, environment_id, name),
    FOREIGN KEY (workspace_id, environment_id) REFERENCES control.environments(workspace_id, environment_id)
);

CREATE TABLE control.credentials (
    credential_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    agent_id uuid NOT NULL REFERENCES control.agents(agent_id),
    secret_hash text NOT NULL,
    scopes text[] NOT NULL,
    expires_at timestamptz,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE policy.policy_versions (
    policy_id uuid NOT NULL,
    version integer NOT NULL CHECK (version > 0),
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    environment_id uuid NOT NULL REFERENCES control.environments(environment_id),
    status text NOT NULL CHECK (status IN ('draft', 'published', 'superseded', 'frozen')),
    content_hash bytea NOT NULL,
    rules jsonb NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    published_at timestamptz,
    PRIMARY KEY (policy_id, version),
    UNIQUE (policy_id, content_hash)
);

CREATE TABLE policy.active_policies (
    policy_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    environment_id uuid NOT NULL REFERENCES control.environments(environment_id),
    active_version integer NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (policy_id, active_version) REFERENCES policy.policy_versions(policy_id, version)
);

CREATE TABLE policy.merchants (
    merchant_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    origin text NOT NULL,
    path_prefix text,
    pay_to text,
    network text,
    asset text,
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, origin, path_prefix, pay_to, network, asset)
);

CREATE TABLE payments.payment_intents (
    intent_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    environment_id uuid NOT NULL REFERENCES control.environments(environment_id),
    agent_id uuid NOT NULL REFERENCES control.agents(agent_id),
    idempotency_key text NOT NULL,
    intent_hash bytea NOT NULL,
    merchant jsonb NOT NULL,
    amount_atomic numeric(78, 0) NOT NULL CHECK (amount_atomic >= 0),
    asset text NOT NULL,
    network text NOT NULL,
    status text NOT NULL CHECK (status IN ('evaluated', 'approved', 'denied', 'expired')),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    UNIQUE (workspace_id, agent_id, idempotency_key)
);

CREATE TABLE payments.decisions (
    decision_id uuid PRIMARY KEY,
    intent_id uuid NOT NULL UNIQUE REFERENCES payments.payment_intents(intent_id),
    policy_id uuid NOT NULL,
    policy_version integer NOT NULL,
    outcome text NOT NULL CHECK (outcome IN ('allow', 'deny', 'approval_required')),
    reason_code text NOT NULL,
    explanation text NOT NULL,
    token_key_id text,
    token_hash bytea,
    expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (policy_id, policy_version) REFERENCES policy.policy_versions(policy_id, version)
);

CREATE TABLE payments.budget_counters (
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    agent_id uuid NOT NULL REFERENCES control.agents(agent_id),
    window_type text NOT NULL CHECK (window_type IN ('session', 'daily', 'monthly')),
    window_start timestamptz NOT NULL,
    limit_atomic numeric(78, 0) NOT NULL CHECK (limit_atomic >= 0),
    reserved_atomic numeric(78, 0) NOT NULL DEFAULT 0 CHECK (reserved_atomic >= 0),
    PRIMARY KEY (agent_id, window_type, window_start)
);

CREATE TABLE payments.budget_reservations (
    reservation_id uuid PRIMARY KEY,
    decision_id uuid NOT NULL UNIQUE REFERENCES payments.decisions(decision_id),
    amount_atomic numeric(78, 0) NOT NULL CHECK (amount_atomic >= 0),
    status text NOT NULL CHECK (status IN ('reserved', 'settled', 'failed', 'unknown', 'released')),
    expires_at timestamptz NOT NULL,
    finalized_at timestamptz
);

CREATE TABLE payments.approvals (
    approval_id uuid PRIMARY KEY,
    decision_id uuid NOT NULL UNIQUE REFERENCES payments.decisions(decision_id),
    status text NOT NULL CHECK (status IN ('pending', 'approved', 'denied', 'expired')),
    requested_by text NOT NULL,
    decided_by text,
    expires_at timestamptz NOT NULL,
    decided_at timestamptz
);

CREATE TABLE payments.settlements (
    settlement_id uuid PRIMARY KEY,
    intent_id uuid NOT NULL REFERENCES payments.payment_intents(intent_id),
    decision_id uuid NOT NULL REFERENCES payments.decisions(decision_id),
    transaction_signature text,
    facilitator_status text NOT NULL,
    observed_amount_atomic numeric(78, 0),
    observed_pay_to text,
    outcome text NOT NULL CHECK (outcome IN ('settled', 'failed', 'unknown', 'mismatch')),
    evidence jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (decision_id, transaction_signature)
);

CREATE UNIQUE INDEX settlements_decision_idempotency_idx
    ON payments.settlements (decision_id, (evidence->>'idempotency_key'))
    WHERE evidence->>'idempotency_key' IS NOT NULL;

CREATE TABLE audit.events (
    event_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    aggregate_type text NOT NULL,
    aggregate_id uuid NOT NULL,
    sequence bigint NOT NULL,
    event_type text NOT NULL,
    actor_type text NOT NULL,
    actor_id text NOT NULL,
    payload jsonb NOT NULL,
    prev_hash bytea,
    event_hash bytea NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT now(),
    request_id text NOT NULL,
    UNIQUE (aggregate_id, sequence)
);

CREATE INDEX payment_intents_workspace_created_idx ON payments.payment_intents (workspace_id, created_at DESC);
CREATE INDEX payment_intents_agent_created_idx ON payments.payment_intents (agent_id, created_at DESC);
CREATE INDEX decisions_outcome_created_idx ON payments.decisions (outcome, created_at DESC);
CREATE INDEX settlements_decision_created_idx ON payments.settlements (decision_id, created_at DESC);
CREATE INDEX audit_workspace_occurred_idx ON audit.events (workspace_id, occurred_at DESC);
CREATE INDEX audit_aggregate_sequence_idx ON audit.events (aggregate_id, sequence);

CREATE OR REPLACE FUNCTION policy.reject_published_mutation() RETURNS trigger AS $$
BEGIN
    IF OLD.status IN ('published', 'superseded', 'frozen') THEN
        RAISE EXCEPTION 'published policy versions are immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER policy_versions_immutable
    BEFORE UPDATE OR DELETE ON policy.policy_versions
    FOR EACH ROW EXECUTE FUNCTION policy.reject_published_mutation();

CREATE OR REPLACE FUNCTION audit.reject_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit events are append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_events_append_only
    BEFORE UPDATE OR DELETE ON audit.events
    FOR EACH ROW EXECUTE FUNCTION audit.reject_mutation();
