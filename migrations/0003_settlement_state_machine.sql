CREATE OR REPLACE FUNCTION payments.enforce_reservation_transition() RETURNS trigger AS $$
BEGIN
    IF OLD.status = 'reserved' AND NEW.status NOT IN ('settled','failed','unknown','released') THEN
        RAISE EXCEPTION 'invalid reservation transition from reserved to %', NEW.status;
    ELSIF OLD.status = 'unknown' AND NEW.status NOT IN ('unknown','settled','failed','released') THEN
        RAISE EXCEPTION 'invalid reconciliation transition from unknown to %', NEW.status;
    ELSIF OLD.status NOT IN ('reserved','unknown') AND NEW.status <> OLD.status THEN
        RAISE EXCEPTION 'terminal reservation state is immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER budget_reservation_state_machine
    BEFORE UPDATE OF status ON payments.budget_reservations
    FOR EACH ROW EXECUTE FUNCTION payments.enforce_reservation_transition();

CREATE INDEX settlements_unknown_idx
    ON payments.settlements (created_at)
    WHERE outcome IN ('unknown','mismatch');
