use rusqlite::{params, Connection};
use serde_json::json;
use crate::storage::AppDb;
use crate::scanner::arp::normalize_mac;

pub async fn log_event(db: &AppDb, event_type: &str, device_id: Option<i64>, details: &str) {
    let type_str = event_type.to_string();
    let detail_str = details.to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let _ = db.execute(move |conn| {
        conn.execute(
            "INSERT INTO device_events (event_type, device_id, timestamp, details) VALUES (?1, ?2, ?3, ?4)",
            params![type_str, device_id, now, detail_str],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }).await;
}

pub struct OnlineRecord<'a> {
    pub scan_id: &'a str,
    pub scanned_at: i64,
    pub device_id: i64,
    pub ip: &'a str,
    pub latency_ms: Option<f64>,
    pub network_id: Option<i64>,
    pub rssi: Option<i32>,
}

pub fn record_device_online(
    conn: &Connection,
    record: OnlineRecord,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO scan_history (scan_id, scanned_at, device_id, ip, is_online, latency_ms, network_id, rssi)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)",
        params![
            record.scan_id,
            record.scanned_at,
            record.device_id,
            record.ip,
            record.latency_ms,
            record.network_id,
            record.rssi
        ],
    )
    .map_err(|e| format!("record history: {e}"))?;
    Ok(())
}

pub fn record_new_device_event(
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
    .map_err(|e| format!("record event: {e}"))?;
    Ok(())
}

pub fn get_device_rssi_history(
    conn: &Connection,
    mac: &str,
    since_ms: i64,
) -> Result<Vec<(i64, i32)>, String> {
    let normalized = normalize_mac(mac);
    let mut stmt = conn.prepare(
        "SELECT h.scanned_at, h.rssi 
         FROM scan_history h
         JOIN devices d ON h.device_id = d.id
         WHERE d.mac = ?1 AND h.scanned_at >= ?2 AND h.rssi IS NOT NULL
         ORDER BY h.scanned_at ASC"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![normalized, since_ms], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?))
    }).map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }
    Ok(results)
}

