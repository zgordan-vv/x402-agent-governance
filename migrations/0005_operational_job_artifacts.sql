CREATE TABLE jobs.export_artifacts (
    export_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    status text NOT NULL CHECK (status IN ('ready','failed')),
    privacy_profile text NOT NULL CHECK (privacy_profile IN ('internal','shareable')),
    payload jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE jobs.metric_rollups (
    workspace_id uuid NOT NULL REFERENCES control.workspaces(workspace_id),
    bucket_start timestamptz NOT NULL,
    decisions_total bigint NOT NULL DEFAULT 0,
    settlements_total bigint NOT NULL DEFAULT 0,
    unknown_settlements bigint NOT NULL DEFAULT 0,
    PRIMARY KEY (workspace_id, bucket_start)
);
