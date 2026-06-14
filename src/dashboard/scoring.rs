use std::time::Duration;
use log::{info, error};
use crate::AppState;
use crate::types::SystemStatus;


pub async fn run(state: AppState) {
    info!("[SCORING] Starting background scoring task (60s interval)");
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        if let Err(e) = update_system_score(state.clone()).await {
            error!("[SCORING] Failed to update system score: {e}");
        }
    }
}

async fn update_system_score(state: AppState) -> Result<(), String> {
    let db = state.db.clone();
    
    let res: Result<(i32, i32, i32), String> = db.execute(move |conn| -> Result<(i32, i32, i32), String> {
        // 1. Performance Score
        let performance = {
            let speed: Option<f64> = conn.query_row(
                "SELECT download_mbps FROM speed_tests ORDER BY tested_at DESC LIMIT 1",
                [],
                |row| row.get(0)
            ).ok();
            
            match speed {
                Some(s) if s > 100.0 => 40,
                Some(s) if s > 50.0 => 20,
                Some(_) => 10,
                None => 0,
            }
        };

        // 2. Latency Score
        let latency = {
            let avg_latency: Option<f64> = conn.query_row(
                "SELECT AVG(latency_ms) FROM scan_history WHERE scan_id = (SELECT scan_id FROM scan_history ORDER BY scanned_at DESC LIMIT 1)",
                [],
                |row| row.get(0)
            ).ok().flatten();

            match avg_latency {
                Some(lat) if lat < 20.0 => 30,
                Some(lat) if lat < 50.0 => 15,
                Some(lat) if lat > 100.0 => 0,
                Some(_) => 5,
                None => 0,
            }
        };

        // 3. Security Score
        let security = {
            let counts: Option<(i64, i64)> = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN vendor = 'Unknown' OR vendor IS NULL THEN 1 ELSE 0 END), 0) FROM devices WHERE is_active = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?))
            ).ok();

            match counts {
                Some((total, risky)) if total > 0 => {
                    let recognized = total - risky;
                    let recognized_ratio = recognized as f64 / total as f64;
                    if recognized_ratio > 0.5 || risky <= 3 {
                        30
                    } else {
                        (recognized_ratio * 30.0) as i32
                    }
                }
                _ => 30,
            }
        };

        Ok((performance, latency, security))
    }).await;

    let (performance, latency, security) = res?;
    
    let total = performance + latency + security;
    let now = crate::storage::now_ms();
    
    let status = SystemStatus {
        score: total,
        performance_score: performance,
        latency_score: latency,
        security_score: security,
        last_updated: now,
    };
    
    crate::storage::system_status::save_system_status(&db, status).await?;
    
    info!("[SCORING] System score updated: {total} (P:{performance} L:{latency} S:{security})");
    Ok(())
}

