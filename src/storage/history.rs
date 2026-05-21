use libsql::{params, Connection};
use serde_json::{json, Value};
use crate::storage::AppDb;

pub async fn get_device_history(
    db: AppDb,
    mac: String,
    limit: i64,
) -> Result<Vec<Value>, String> {
    let conn = db.connect().await?;
    let mut rows = conn.query(
        "SELECT h.scan_id, h.scanned_at, h.ip, h.is_online, h.latency_ms, h.open_ports
         FROM scan_history h 
         JOIN devices d ON h.device_id = d.id 
         WHERE d.mac = ?1 
         ORDER BY h.scanned_at DESC 
         LIMIT ?2",
        params![mac, limit]
    ).await.map_err(|e| format!("query history: {e}"))?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        results.push(json!({
            "scan_id": row.get::<Option<String>>(0).map_err(|e| e.to_string())?.unwrap_or_default(),
            "scanned_at": row.get::<Option<i64>>(1).map_err(|e| e.to_string())?.unwrap_or_default(),
            "ip": row.get::<Option<String>>(2).map_err(|e| e.to_string())?.unwrap_or_default(),
            "is_online": row.get::<Option<i64>>(3).map_err(|e| e.to_string())?.unwrap_or_default() != 0,
            "latency_ms": row.get::<Option<f64>>(4).map_err(|e| e.to_string())?,
            "open_ports": row.get::<Option<String>>(5).map_err(|e| e.to_string())?,
        }));
    }

    Ok(results)
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

pub async fn list_history(
    conn: &Connection,
    mac: Option<&str>,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    limit: i64,
) -> Result<Vec<Value>, String> {
    let mut q = String::from(
        "SELECT h.scan_id, h.scanned_at, h.ip, h.is_online, h.latency_ms, d.mac
         FROM scan_history h JOIN devices d ON d.id = h.device_id
         WHERE 1=1",
    );
    if mac.is_some() {
        q.push_str(" AND d.mac = ?1");
    }
    if from_ms.is_some() {
        q.push_str(" AND h.scanned_at >= ?2");
    }
    if to_ms.is_some() {
        q.push_str(" AND h.scanned_at <= ?3");
    }
    q.push_str(" ORDER BY h.scanned_at DESC LIMIT ?4");

    let mut rows = conn.query(&q, params![
        mac.unwrap_or(""),
        from_ms.unwrap_or(0),
        to_ms.unwrap_or(i64::MAX),
        limit,
    ]).await.map_err(|e| format!("query history: {e}"))?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        results.push(json!({
            "scanId":    row.get::<Option<String>>(0).map_err(|e| e.to_string())?.unwrap_or_default(),
            "scannedAt": row.get::<Option<i64>>(1).map_err(|e| e.to_string())?.unwrap_or_default(),
            "ip":        row.get::<Option<String>>(2).map_err(|e| e.to_string())?.unwrap_or_default(),
            "isOnline":  row.get::<Option<i64>>(3).map_err(|e| e.to_string())?.unwrap_or_default() != 0,
            "latencyMs": row.get::<Option<f64>>(4).map_err(|e| e.to_string())?,
            "mac":       row.get::<Option<String>>(5).map_err(|e| e.to_string())?.unwrap_or_default(),
        }));
    }

    Ok(results)
}

pub async fn list_events(conn: &Connection, limit: i64) -> Result<Vec<Value>, String> {
    let mut rows = conn
        .query(
            "SELECT e.id, e.event_type, e.timestamp, e.details, d.mac
             FROM device_events e LEFT JOIN devices d ON d.id = e.device_id
             ORDER BY e.timestamp DESC LIMIT ?1",
            params![limit]
        )
        .await
        .map_err(|e| format!("query events: {e}"))?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        results.push(json!({
            "id":        row.get::<Option<i64>>(0).map_err(|e| e.to_string())?.unwrap_or_default(),
            "eventType": row.get::<Option<String>>(1).map_err(|e| e.to_string())?.unwrap_or_default(),
            "timestamp": row.get::<Option<i64>>(2).map_err(|e| e.to_string())?.unwrap_or_default(),
            "details":   row.get::<Option<String>>(3).map_err(|e| e.to_string())?,
            "mac":       row.get::<Option<String>>(4).map_err(|e| e.to_string())?,
        }));
    }

    Ok(results)
}
