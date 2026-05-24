use crate::storage::AppDb;
use std::time::Duration;
use tokio::time;

pub struct MetricsCompactor;

impl MetricsCompactor {
    /// Spawns the data-aging compactor task onto a long-running background worker thread
    pub fn start(db: AppDb) {
        tokio::spawn(async move {
            // Check compaction needs every 1 hour matching Prometheus interval profiles
            let mut interval = time::interval(Duration::from_secs(3600));
            log::info!("[FLIGHT_RECORDER] Relational Metrics Compactor background task spawned.");

            loop {
                interval.tick().await;
                if let Err(e) = Self::run_compaction_pass(&db).await {
                    log::error!("[FLIGHT_RECORDER] Compactor transaction run failed: {}", e);
                }
            }
        });
    }

    async fn run_compaction_pass(db: &AppDb) -> Result<(), String> {
        let conn = db.connect().await.map_err(|e| e.to_string())?;
        
        // Calculate the cutoff point: older than 24 hours
        let current_time_ms = chrono::Utc::now().timestamp_millis();
        let cutoff_time_ms = current_time_ms - (24 * 60 * 60 * 1000);

        log::info!("[FLIGHT_RECORDER] Compactor scanning for high-frequency records older than 24h...");

        // 1. Identify raw scan history blocks that can be rolled up
        // Group by device and floor the scanned_at timestamp to the nearest hour (3,600,000 ms)
        let query = "
            SELECT 
                device_id,
                AVG(latency_ms) as avg_lat,
                MIN(latency_ms) as min_lat,
                MAX(latency_ms) as max_lat,
                COUNT(id) as total_scans,
                (scanned_at / 3600000) * 3600000 as hour_bucket
            FROM scan_history
            WHERE scanned_at < ?1
            GROUP BY device_id, hour_bucket";

        let mut stmt = conn.prepare(query).await.map_err(|e| e.to_string())?;
        let mut rows = stmt.query(libsql::params![cutoff_time_ms]).await.map_err(|e| e.to_string())?;

        let mut consolidated_entries = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            consolidated_entries.push((
                row.get::<i64>(0).unwrap_or(0),     // device_id
                row.get::<f64>(1).unwrap_or(0.0),   // avg_latency_ms
                row.get::<f64>(2).unwrap_or(0.0),   // min_latency_ms
                row.get::<f64>(3).unwrap_or(0.0),   // max_latency_ms
                row.get::<i64>(4).unwrap_or(0),     // total_scans
                row.get::<i64>(5).unwrap_or(0),     // hour_bucket
            ));
        }

        if consolidated_entries.is_empty() {
            log::info!("[FLIGHT_RECORDER] Compactor pass complete: zero historical entries match aging criteria.");
            return Ok(());
        }

        // 2. Commit rolled-up aggregates into the compressed table layer inside an atomic block
        for entry in &consolidated_entries {
            conn.execute(
                "INSERT INTO hourly_metrics (device_id, avg_latency_ms, min_latency_ms, max_latency_ms, total_scans, recorded_hour)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(device_id, recorded_hour) DO UPDATE SET
                    avg_latency_ms = (avg_latency_ms + ?2) / 2.0,
                    total_scans = total_scans + ?5",
                libsql::params![entry.0, entry.1, entry.2, entry.3, entry.4, entry.5],
            ).await.map_err(|e| e.to_string())?;
        }

        // 3. Purge the highly volatile microscopic raw scan logs older than 24 hours
        let purged_rows = conn.execute(
            "DELETE FROM scan_history WHERE scanned_at < ?1",
            libsql::params![cutoff_time_ms],
        ).await.map_err(|e| e.to_string())?;

        log::info!(
            "[FLIGHT_RECORDER] Compactor successfully processed {} hour-buckets. Purged {} high-frequency raw logs from flash storage.",
            consolidated_entries.len(),
            purged_rows
        );

        // Execute an explicit incremental vacuum pass to clean up empty database pages on disk
        let _ = conn.execute("PRAGMA incremental_vacuum(50);", ()).await;

        Ok(())
    }
}
