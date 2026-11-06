CREATE UNIQUE INDEX jobs_type_scope_unique_idx
    ON jobs.work_items (job_type, (payload->>'scope'))
    WHERE payload ? 'scope';
