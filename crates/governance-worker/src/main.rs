use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::time::Duration;

async fn reconcile_settlement(
    pool: &PgPool,
    payload: &serde_json::Value,
) -> Result<bool, sqlx::Error> {
    let settlement_id = payload
        .get("settlement_id")
        .and_then(|v| v.as_str())
        .and_then(|v| uuid::Uuid::parse_str(v).ok());
    let Some(settlement_id) = settlement_id else {
        return Ok(false);
    };
    let mut tx = pool.begin().await?;
    let resolution: Option<String> = sqlx::query_scalar("SELECT evidence->>'resolution' FROM payments.settlements WHERE settlement_id=$1 AND outcome IN ('unknown','mismatch') FOR UPDATE").bind(settlement_id).fetch_optional(&mut *tx).await?;
    let Some(resolution) = resolution else {
        tx.rollback().await?;
        return Ok(false);
    };
    if resolution != "settled" && resolution != "failed" {
        tx.rollback().await?;
        return Ok(false);
    }
    let verified: bool=sqlx::query_scalar("SELECT COALESCE((evidence->>'resolution_verified')::boolean,false) FROM payments.settlements WHERE settlement_id=$1").bind(settlement_id).fetch_one(&mut *tx).await?;
    if !verified {
        tx.rollback().await?;
        return Ok(false);
    }
    let reservation_status = resolution.as_str();
    sqlx::query("UPDATE payments.settlements SET outcome=$1 WHERE settlement_id=$2")
        .bind(&resolution)
        .bind(settlement_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE payments.budget_reservations SET status=$1, finalized_at=now() WHERE decision_id=(SELECT decision_id FROM payments.settlements WHERE settlement_id=$2) AND status='unknown'").bind(reservation_status).bind(settlement_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

async fn lease_one(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query_as::<_, (uuid::Uuid,String,serde_json::Value,i32)>(
        "UPDATE jobs.work_items SET status = 'leased', attempts = attempts + 1, lease_until = now() + interval '60 seconds', updated_at = now() WHERE job_id = (SELECT job_id FROM jobs.work_items WHERE status IN ('pending','retryable') AND available_at <= now() ORDER BY available_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING job_id, job_type, payload, attempts"
    ).fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    if let Some((job_id, job_type, payload, _attempts)) = row {
        tracing::info!(%job_id, %job_type, "leased job");
        let handled = if job_type == "settlement_reconciliation" {
            reconcile_settlement(pool, &payload).await?
        } else {
            false
        };
        if handled {
            sqlx::query("UPDATE jobs.work_items SET status='complete', lease_until=NULL, updated_at=now() WHERE job_id=$1").bind(job_id).execute(pool).await?;
        } else {
            sqlx::query("UPDATE jobs.work_items SET status=CASE WHEN attempts >= 5 THEN 'dead_letter' ELSE 'retryable' END, available_at=now()+interval '30 seconds', lease_until=NULL, last_error=$2, updated_at=now() WHERE job_id=$1").bind(job_id).bind(if job_type=="settlement_reconciliation" {"resolution not confirmed"} else {"handler not registered"}).execute(pool).await?;
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect_lazy(&database_url)
        .expect("valid DATABASE_URL");
    let mut ticker = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            _ = ticker.tick() => { if let Err(error) = lease_one(&pool).await { tracing::warn!(%error, "job poll failed"); } }
            result = tokio::signal::ctrl_c() => { if let Err(error) = result { tracing::warn!(%error, "shutdown signal failed"); } break; }
        }
    }
    pool.close().await;
}
