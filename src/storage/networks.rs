use crate::types::NetworkRecord;
use rusqlite::{params, Connection, Row};

/// Insert or update a network record identified by its BSSID (or gateway MAC on wired).
/// Returns the `networks.id` for the upserted row.
pub fn upsert_network(
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
    .map_err(|e| format!("upsert network: {e}"))?;

    let id: i64 = conn.query_row(
        "SELECT id FROM networks WHERE bssid = ?1",
        params![bssid],
        |row| row.get(0)
    ).map_err(|e| format!("network id lookup: {e}"))?;

    Ok(id)
}

pub fn list_networks(conn: &Connection) -> Result<Vec<NetworkRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.ssid, n.bssid, n.gateway, n.subnet, n.first_seen, n.last_seen,
                    COUNT(DISTINCT d.id) AS device_count
             FROM networks n
             LEFT JOIN devices d ON d.network_id = n.id
             GROUP BY n.id
             ORDER BY n.last_seen DESC"
        )
        .map_err(|e| format!("prepare networks query: {e}"))?;

    let rows = stmt.query_map([], |row| {
        row_to_network(row)
    }).map_err(|e| format!("query networks: {e}"))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }

    Ok(results)
}

fn row_to_network(row: &Row) -> Result<NetworkRecord, rusqlite::Error> {
    Ok(NetworkRecord {
        id:           row.get::<_, Option<i64>>(0)?.unwrap_or_default(),
        ssid:         row.get::<_, Option<String>>(1)?,
        bssid:        row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        gateway:      row.get::<_, Option<String>>(3)?,
        subnet:       row.get::<_, Option<String>>(4)?,
        first_seen:   row.get::<_, Option<i64>>(5)?.unwrap_or_default(),
        last_seen:    row.get::<_, Option<i64>>(6)?.unwrap_or_default(),
        device_count: row.get::<_, Option<i64>>(7)?.unwrap_or_default(),
    })
}

