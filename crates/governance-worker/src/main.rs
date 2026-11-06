use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::time::Duration;

const MAX_ATTEMPTS: i32 = 5;
fn backoff_seconds(attempt: i32) -> i32 {
    2_i32.saturating_pow(attempt.clamp(0, 9) as u32).min(300)
}

async fn ensure_maintenance_jobs(pool: &PgPool) -> Result<(), sqlx::Error> {
    for job_type in [
        "approval_expiry",
        "reservation_expiry",
        "retention_cleanup",
        "metric_rollup",
    ] {
        sqlx::query("INSERT INTO jobs.work_items(job_id,job_type,payload) VALUES($1,$2,jsonb_build_object('scope','global')) ON CONFLICT DO NOTHING")
            .bind(uuid::Uuid::now_v7()).bind(job_type).execute(pool).await?;
    }
    Ok(())
}

async fn retention_cleanup(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM payments.decision_token_nonces WHERE expires_at < now() - interval '1 day'",
    )
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM jobs.work_items WHERE status IN ('complete','dead_letter') AND updated_at < now() - interval '30 days'").execute(pool).await?;
    Ok(())
}

async fn metric_rollup(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO jobs.metric_rollups(workspace_id,bucket_start,decisions_total,settlements_total,unknown_settlements) SELECT w.workspace_id,date_trunc('hour',now()),(SELECT count(*) FROM payments.decisions d JOIN payments.payment_intents i ON i.intent_id=d.intent_id WHERE i.workspace_id=w.workspace_id),(SELECT count(*) FROM payments.settlements s JOIN payments.payment_intents i ON i.intent_id=s.intent_id WHERE i.workspace_id=w.workspace_id),(SELECT count(*) FROM payments.settlements s JOIN payments.payment_intents i ON i.intent_id=s.intent_id WHERE i.workspace_id=w.workspace_id AND s.outcome IN ('unknown','mismatch')) FROM control.workspaces w ON CONFLICT(workspace_id,bucket_start) DO UPDATE SET decisions_total=EXCLUDED.decisions_total,settlements_total=EXCLUDED.settlements_total,unknown_settlements=EXCLUDED.unknown_settlements").execute(pool).await?;
    Ok(())
}

async fn generate_export(pool: &PgPool, payload: &serde_json::Value) -> Result<bool, sqlx::Error> {
    let Some(export_id) = payload
        .get("export_id")
        .and_then(|v| v.as_str())
        .and_then(|v| uuid::Uuid::parse_str(v).ok())
    else {
        return Ok(false);
    };
    let Some(workspace_id) = payload
        .get("workspace_id")
        .and_then(|v| v.as_str())
        .and_then(|v| uuid::Uuid::parse_str(v).ok())
    else {
        return Ok(false);
    };
    let profile = payload
        .get("privacy_profile")
        .and_then(|v| v.as_str())
        .unwrap_or("shareable");
    sqlx::query("INSERT INTO jobs.export_artifacts(export_id,workspace_id,status,privacy_profile,payload) VALUES($1,$2,'ready',$3,$4) ON CONFLICT DO NOTHING").bind(export_id).bind(workspace_id).bind(profile).bind(payload).execute(pool).await?;
    Ok(true)
}

async fn expire_approvals(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE payments.approvals SET status='expired', decided_at=now() WHERE status='pending' AND expires_at <= now()").execute(pool).await?;
    Ok(())
}

async fn expire_reservations(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let rows=sqlx::query_as::<_,(uuid::Uuid,uuid::Uuid,String)>("UPDATE payments.budget_reservations SET status='released', finalized_at=now() WHERE reservation_id IN (SELECT reservation_id FROM payments.budget_reservations WHERE status='reserved' AND expires_at<=now() FOR UPDATE SKIP LOCKED) RETURNING reservation_id,decision_id,amount_atomic::text").fetch_all(&mut *tx).await?;
    for (_, decision_id, amount) in rows {
        sqlx::query("UPDATE payments.budget_counters c SET reserved_atomic=GREATEST(0,c.reserved_atomic-$1::numeric) WHERE c.agent_id=(SELECT agent_id FROM payments.payment_intents i JOIN payments.decisions d ON d.intent_id=i.intent_id WHERE d.decision_id=$2) AND c.window_type='daily' AND c.window_start=date_trunc('day',now())").bind(amount).bind(decision_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn reconcile_settlement(
    pool: &PgPool,
    payload: &serde_json::Value,
) -> Result<bool, sqlx::Error> {
    let Some(settlement_id) = payload
        .get("settlement_id")
        .and_then(|v| v.as_str())
        .and_then(|v| uuid::Uuid::parse_str(v).ok())
    else {
        return Ok(false);
    };
    let mut tx = pool.begin().await?;
    let row=sqlx::query_as::<_,(String,bool)>("SELECT evidence->>'resolution',COALESCE((evidence->>'resolution_verified')::boolean,false) FROM payments.settlements WHERE settlement_id=$1 AND outcome IN ('unknown','mismatch') FOR UPDATE").bind(settlement_id).fetch_optional(&mut *tx).await?;
    let Some((resolution, verified)) = row else {
        tx.rollback().await?;
        return Ok(false);
    };
    if !verified || (resolution != "settled" && resolution != "failed") {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query("UPDATE payments.settlements SET outcome=$1 WHERE settlement_id=$2")
        .bind(&resolution)
        .bind(settlement_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE payments.budget_reservations SET status=$1,finalized_at=now() WHERE decision_id=(SELECT decision_id FROM payments.settlements WHERE settlement_id=$2) AND status='unknown'").bind(&resolution).bind(settlement_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

async fn dispatch(
    pool: &PgPool,
    job_type: &str,
    payload: &serde_json::Value,
) -> Result<bool, sqlx::Error> {
    match job_type {
        "approval_expiry" => {
            expire_approvals(pool).await?;
            Ok(true)
        }
        "reservation_expiry" => {
            expire_reservations(pool).await?;
            Ok(true)
        }
        "retention_cleanup" => {
            retention_cleanup(pool).await?;
            Ok(true)
        }
        "metric_rollup" => {
            metric_rollup(pool).await?;
            Ok(true)
        }
        "export_generation" => generate_export(pool, payload).await,
        "settlement_reconciliation" => reconcile_settlement(pool, payload).await,
        _ => Ok(false),
    }
}

async fn lease_one(pool: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query("UPDATE jobs.work_items SET status='retryable',lease_until=NULL WHERE status='leased' AND lease_until<now()").execute(pool).await?;
    let mut tx = pool.begin().await?;
    let row=sqlx::query_as::<_,(uuid::Uuid,String,serde_json::Value,i32)>("UPDATE jobs.work_items SET status='leased',attempts=attempts+1,lease_until=now()+interval '60 seconds',updated_at=now() WHERE job_id=(SELECT job_id FROM jobs.work_items WHERE status IN ('pending','retryable') AND available_at<=now() ORDER BY available_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING job_id,job_type,payload,attempts").fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    let Some((job_id, job_type, payload, attempts)) = row else {
        return Ok(false);
    };
    let handled = dispatch(pool, &job_type, &payload).await?;
    if handled {
        sqlx::query("UPDATE jobs.work_items SET status='complete',lease_until=NULL,updated_at=now() WHERE job_id=$1").bind(job_id).execute(pool).await?;
    } else {
        sqlx::query("UPDATE jobs.work_items SET status=CASE WHEN attempts>=$2 THEN 'dead_letter' ELSE 'retryable' END,available_at=now()+make_interval(secs=>$3),lease_until=NULL,last_error=$4,updated_at=now() WHERE job_id=$1").bind(job_id).bind(MAX_ATTEMPTS).bind(backoff_seconds(attempts)).bind(if job_type=="settlement_reconciliation"{"resolution not confirmed"}else{"handler not registered"}).execute(pool).await?;
    }
    Ok(true)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect_lazy(&url)
        .expect("valid DATABASE_URL");
    let mut ticker = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {_ = ticker.tick()=>{if let Err(e)=ensure_maintenance_jobs(&pool).await{tracing::warn!(%e,"maintenance scheduling failed")}if let Err(e)=lease_one(&pool).await{tracing::warn!(%e,"job poll failed")}},result=tokio::signal::ctrl_c()=>{if let Err(e)=result{tracing::warn!(%e,"shutdown signal failed")}break}}
    }
    pool.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backoff_is_bounded_and_exponential() {
        assert_eq!(backoff_seconds(0), 1);
        assert_eq!(backoff_seconds(3), 8);
        assert_eq!(backoff_seconds(99), 300);
    }
}
