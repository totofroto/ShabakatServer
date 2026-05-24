use libsql::params;
use crate::storage::AppDb;

pub struct MetricEntry {
    pub scan_id: String,
    pub scanned_at: i64,
    pub device_id: i64,
    pub ip: String,
    pub is_online: bool,
    pub latency_ms: Option<f64>,
    pub open_ports: Option<String>,
}

pub struct MetricStorage;

impl MetricStorage {
    pub async fn log_heartbeat_batch(db: &AppDb, batch: Vec<MetricEntry>) -> Result<(), String> {
        let conn = db.connect().await?;
        conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;

        for entry in batch {
            let res = conn.execute(
                "INSERT INTO scan_history (scan_id, scanned_at, device_id, ip, is_online, latency_ms, open_ports)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    entry.scan_id,
                    entry.scanned_at,
                    entry.device_id,
                    entry.ip,
                    if entry.is_online { 1 } else { 0 },
                    entry.latency_ms,
                    entry.open_ports,
                ],
            ).await;

            if let Err(e) = res {
                let _ = conn.execute("ROLLBACK", ()).await;
                return Err(format!("failed to insert heartbeat: {}", e));
            }
        }

        conn.execute("COMMIT", ()).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
