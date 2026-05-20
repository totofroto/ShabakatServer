use crate::types::NetworkRecord;
use libsql::{params, Connection, Row};

/// Insert or update a network record identified by its BSSID (or gateway MAC on wired).
/// Returns the `networks.id` for the upserted row.
pub async fn upsert_network(
    conn: &Connection,
    ssid: Option<&str>,
    bssid: &str,
    gateway: Option<&str>,
    subnet: Option<&str>,
    now_ms: i64,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO networks (ssid, bssid, gateway, subnet, first_seen, last_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(bssid) DO UPDATE SET
             ssid      = COALESCE(?1, ssid),
             gateway   = COALESCE(?3, gateway),
             subnet    = COALESCE(?4, subnet),
             last_seen = ?5",
        params![ssid, bssid, gateway, subnet, now_ms],
    )
    .await
    .map_err(|e| format!("upsert network: {e}"))?;

    let mut rows = conn.query(
        "SELECT id FROM networks WHERE bssid = ?1",
        params![bssid],
    ).await.map_err(|e| format!("network id lookup: {e}"))?;

    if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        Ok(row.get::<Option<i64>>(0).map_err(|e| e.to_string())?.unwrap_or_default())
    } else {
        Err("network id not found after upsert".to_string())
    }
}

pub async fn list_networks(conn: &Connection) -> Result<Vec<NetworkRecord>, String> {
    let mut rows = conn
        .query(
            "SELECT n.id, n.ssid, n.bssid, n.gateway, n.subnet, n.first_seen, n.last_seen,
                    COUNT(DISTINCT d.id) AS device_count
             FROM networks n
             LEFT JOIN devices d ON d.network_id = n.id
             GROUP BY n.id
             ORDER BY n.last_seen DESC",
            ()
        )
        .await
        .map_err(|e| format!("query networks: {e}"))?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        results.push(row_to_network(&row).map_err(|e| e.to_string())?);
    }

    Ok(results)
}

fn row_to_network(row: &Row) -> Result<NetworkRecord, libsql::Error> {
    Ok(NetworkRecord {
        id:           row.get::<Option<i64>>(0)?.unwrap_or_default(),
        ssid:         row.get::<Option<String>>(1)?,
        bssid:        row.get::<Option<String>>(2)?.unwrap_or_default(),
        gateway:      row.get::<Option<String>>(3)?,
        subnet:       row.get::<Option<String>>(4)?,
        first_seen:   row.get::<Option<i64>>(5)?.unwrap_or_default(),
        last_seen:    row.get::<Option<i64>>(6)?.unwrap_or_default(),
        device_count: row.get::<Option<i64>>(7)?.unwrap_or_default(),
    })
}
