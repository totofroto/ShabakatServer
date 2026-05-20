use crate::storage::AppDb;
use crate::types::SystemStatus;
use libsql::params;

pub async fn save_system_status(db: &AppDb, status: SystemStatus) -> Result<(), String> {
    let conn = db.connect().await?;
    conn.execute(
        "INSERT INTO system_status (id, score, performance_score, latency_score, security_score, last_updated)
         VALUES (1, ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            score = excluded.score,
            performance_score = excluded.performance_score,
            latency_score = excluded.latency_score,
            security_score = excluded.security_score,
            last_updated = excluded.last_updated",
        params![
            status.score,
            status.performance_score,
            status.latency_score,
            status.security_score,
            status.last_updated
        ],
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_system_status(db: &AppDb) -> Result<Option<SystemStatus>, String> {
    let conn = db.connect().await?;
    let mut rows = conn.query("SELECT score, performance_score, latency_score, security_score, last_updated FROM system_status WHERE id = 1", ()).await.map_err(|e| e.to_string())?;

    if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        Ok(Some(SystemStatus {
            score: row.get(0).map_err(|e| e.to_string())?,
            performance_score: row.get(1).map_err(|e| e.to_string())?,
            latency_score: row.get(2).map_err(|e| e.to_string())?,
            security_score: row.get(3).map_err(|e| e.to_string())?,
            last_updated: row.get(4).map_err(|e| e.to_string())?,
        }))
    } else {
        Ok(None)
    }
}
