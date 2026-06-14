use crate::storage::AppDb;
use crate::types::SystemStatus;
use rusqlite::params;

pub async fn save_system_status(db: &AppDb, status: SystemStatus) -> Result<(), String> {
    db.execute(move |conn| -> Result<(), String> {
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
        .map_err(|e| e.to_string())?;
        Ok(())
    }).await
}

pub async fn get_system_status(db: &AppDb) -> Result<Option<SystemStatus>, String> {
    db.execute(move |conn| -> Result<Option<SystemStatus>, String> {
        let status = conn.query_row(
            "SELECT score, performance_score, latency_score, security_score, last_updated FROM system_status WHERE id = 1",
            [],
            |row| {
                Ok(SystemStatus {
                    score: row.get(0)?,
                    performance_score: row.get(1)?,
                    latency_score: row.get(2)?,
                    security_score: row.get(3)?,
                    last_updated: row.get(4)?,
                })
            }
        ).ok();
        Ok(status)
    }).await
}

