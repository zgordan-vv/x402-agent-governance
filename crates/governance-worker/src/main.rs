use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::time::Duration;

async fn lease_one(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE jobs.work_items SET status = 'leased', attempts = attempts + 1, lease_until = now() + interval '60 seconds', updated_at = now() WHERE job_id = (SELECT job_id FROM jobs.work_items WHERE status IN ('pending','retryable') AND available_at <= now() ORDER BY available_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING job_id"
    ).fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    if let Some(job_id) = row {
        tracing::info!(%job_id, "leased job");
        // Phase 10 will dispatch job_type handlers; an unknown type is safely retried.
        sqlx::query("UPDATE jobs.work_items SET status = CASE WHEN attempts >= 5 THEN 'dead_letter' ELSE 'retryable' END, available_at = now() + interval '30 seconds', lease_until = NULL, last_error = 'handler not registered', updated_at = now() WHERE job_id = $1")
            .bind(job_id).execute(pool).await?;
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
