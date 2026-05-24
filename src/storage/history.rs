use libsql::{params, Connection};
use serde_json::json;
use crate::storage::AppDb;

pub async fn log_event(db: &AppDb, event_type: &str, device_id: Option<i64>, details: &str) {
    let type_str = event_type.to_string();
    let detail_str = details.to_string();
    let now = chrono::Utc::now().timestamp_millis();

    if let Ok(conn) = db.connect().await {
        let _ = conn.execute(
            "INSERT INTO device_events (event_type, device_id, timestamp, details) VALUES (?1, ?2, ?3, ?4)",
            params![type_str, device_id, now, detail_str],
        ).await;
    }
}

pub async fn record_device_online(
    conn: &Connection,
    scan_id: &str,
    scanned_at: i64,
    device_id: i64,
    ip: &str,
    latency_ms: Option<f64>,
    network_id: Option<i64>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO scan_history (scan_id, scanned_at, device_id, ip, is_online, latency_ms, network_id)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
        params![scan_id, scanned_at, device_id, ip, latency_ms, network_id],
    )
    .await
    .map_err(|e| format!("record history: {e}"))?;
    Ok(())
}

pub async fn record_new_device_event(
    conn: &Connection,
    device_id: i64,
    ip: &str,
    mac: &str,
    vendor: Option<&str>,
    now_ms: i64,
) -> Result<(), String> {
    let details = serde_json::to_string(&json!({
        "ip": ip,
        "mac": mac,
        "vendor": vendor,
    }))
    .unwrap_or_default();

    conn.execute(
        "INSERT INTO device_events (event_type, device_id, timestamp, details)
         VALUES ('new_device', ?1, ?2, ?3)",
        params![device_id, now_ms, details],
    )
    .await
    .map_err(|e| format!("record event: {e}"))?;
    Ok(())
}
