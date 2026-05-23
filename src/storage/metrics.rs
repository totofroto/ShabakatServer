use libsql::params;
use log::info;
use crate::storage::AppDb;

pub struct MetricEntry {
    pub id: Option<i64>,
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

    /// Compresses high-frequency raw 20-second heartbeat entries into historical 1-hour buckets,
    /// mimicking Prometheus's leveled compaction passes to protect disk space on your NAS.
    /// Raw entries older than 24 hours are consolidated, and metrics older than retention_days are pruned.
    pub async fn run_leveled_compaction(db: &AppDb, retention_days: i64) -> Result<(), String> {
        info!("[FLIGHT_RECORDER] Initiating leveled compaction pass on high-frequency metrics database...");
        
        let conn = db.connect().await?;
        conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;

        let now = chrono::Utc::now().timestamp_millis();
        let compaction_threshold = now - (24 * 60 * 60 * 1000); // 24 hours ago
        let retention_threshold = now - (retention_days * 24 * 60 * 60 * 1000);

        // 1. Group raw 'heartbeat-%' entries by 1-hour slots and device parameters, then upsert averages
        let aggregation_res = conn.execute(
            "INSERT INTO scan_history (scan_id, scanned_at, device_id, ip, is_online, latency_ms, open_ports)
             SELECT 
                'compacted-hourly' as scan_id,
                (scanned_at / 3600000) * 3600000 as hourly_bucket,
                device_id,
                ip,
                CASE WHEN AVG(is_online) >= 0.5 THEN 1 ELSE 0 END as is_online,
                AVG(latency_ms) as latency_ms,
                '[]' as open_ports
             FROM scan_history
             WHERE scan_id LIKE 'heartbeat-%' AND scanned_at < ?1 AND scanned_at > ?2
             GROUP BY hourly_bucket, device_id, ip",
            params![compaction_threshold, retention_threshold]
        ).await;

        if let Err(e) = aggregation_res {
            let _ = conn.execute("ROLLBACK", ()).await;
            return Err(format!("Compaction aggregation processing step failed: {}", e));
        }

        // 2. Prune the heavy raw granular entries older than 24 hours to clear up space
        let prune_res = conn.execute(
            "DELETE FROM scan_history 
             WHERE scan_id LIKE 'heartbeat-%' AND scanned_at < ?1",
            params![compaction_threshold]
        ).await;

        if let Err(e) = prune_res {
            let _ = conn.execute("ROLLBACK", ()).await;
            return Err(format!("Compaction raw logs data deletion pruning phase failed: {}", e));
        }

        // 3. Enforce the absolute historical long-term retention cutoff window
        let lifecycle_res = conn.execute(
            "DELETE FROM scan_history WHERE scanned_at < ?1",
            params![retention_threshold]
        ).await;

        if let Err(e) = lifecycle_res {
            let _ = conn.execute("ROLLBACK", ()).await;
            return Err(format!("Compaction lifecycle data expiration cleanup failed: {}", e));
        }

        conn.execute("COMMIT", ()).await.map_err(|e| e.to_string())?;
        info!("[FLIGHT_RECORDER] Prometheus-style leveled compaction loop finished successfully.");
        
        Ok(())
    }
}
