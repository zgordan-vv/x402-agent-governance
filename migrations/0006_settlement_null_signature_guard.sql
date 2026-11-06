-- At most one unsigned settlement can exist for a decision. Signed terminal
-- evidence remains unique by (decision_id, transaction_signature).
CREATE UNIQUE INDEX settlements_one_unsigned_per_decision_idx
    ON payments.settlements (decision_id)
    WHERE transaction_signature IS NULL;
