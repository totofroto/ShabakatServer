use std::time::Duration;
use log::{info, error};
use crate::AppState;
use crate::storage::AppDb;
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
    
    // 1. Performance Score (latest speed test)
    let performance = get_performance_score(&db).await?;
    
    // 2. Latency Score (latest scan average)
    let latency = get_latency_score(&db).await?;
    
    // 3. Security Score (device identity)
    let security = get_security_score(&db).await?;
    
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

async fn get_performance_score(db: &AppDb) -> Result<i32, String> {
    let conn = db.connect().await?;
    let mut rows = conn.query("SELECT download_mbps FROM speed_tests ORDER BY tested_at DESC LIMIT 1", ()).await.map_err(|e| e.to_string())?;
    
    if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let speed: f64 = row.get(0).map_err(|e| e.to_string())?;
        if speed > 100.0 { Ok(40) }
        else if speed > 50.0 { Ok(20) }
        else { Ok(10) }
    } else {
        Ok(0)
    }
}

async fn get_latency_score(db: &AppDb) -> Result<i32, String> {
    let conn = db.connect().await?;
    // Get average latency from the latest scan
    let mut rows = conn.query("SELECT AVG(latency_ms) FROM scan_history WHERE scan_id = (SELECT scan_id FROM scan_history ORDER BY scanned_at DESC LIMIT 1)", ()).await.map_err(|e| e.to_string())?;
    
    if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let avg_latency: Option<f64> = row.get(0).map_err(|e| e.to_string())?;
        if let Some(lat) = avg_latency {
            if lat < 20.0 { Ok(30) }
            else if lat < 50.0 { Ok(15) }
            else if lat > 100.0 { Ok(0) }
            else { Ok(5) }
        } else {
            Ok(0)
        }
    } else {
        Ok(0)
    }
}

async fn get_security_score(db: &AppDb) -> Result<i32, String> {
    let conn = db.connect().await?;
    let mut rows = conn.query("SELECT COUNT(*), COALESCE(SUM(CASE WHEN vendor = 'Unknown' OR vendor IS NULL THEN 1 ELSE 0 END), 0) FROM devices WHERE is_active = 1", ()).await.map_err(|e| e.to_string())?;
    
    if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let total: i64 = row.get(0).map_err(|e| e.to_string())?;
        let risky: i64 = row.get(1).map_err(|e| e.to_string())?;
        
        if total == 0 { return Ok(30); }
        
        let recognized = total - risky;
        let recognized_ratio = recognized as f64 / total as f64;
        
        if recognized_ratio > 0.5 || risky <= 3 {
            Ok(30)
        } else {
            Ok((recognized_ratio * 30.0) as i32)
        }
    } else {
        Ok(30)
    }
}
