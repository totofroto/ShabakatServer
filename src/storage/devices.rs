use crate::scanner::fingerprints::{classify_from_hostname, classify_from_vendor};
use crate::storage::AppDb;
use crate::types::{DiscoveredDevice, DeviceRecord};
use libsql::{params, Connection, Row};

pub async fn complete_scan_persistence(
    db: AppDb,
    devices: Vec<DiscoveredDevice>,
    scan_id: String,
    network_id: Option<i64>,
) -> Result<Vec<(String, String, String, String)>, String> {
    let now = super::now_ms();
    let conn = db.connect_dedicated()?;
    
    conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;
    
    let mut new_devices = Vec::new();
    
    for dev in &devices {
        match upsert_discovered_device(&conn, dev, now, network_id).await {
            Ok((device_id, is_new)) => {
                // Record history
                if let Err(e) = super::history::record_device_online(
                    &conn,
                    &scan_id,
                    now,
                    device_id,
                    &dev.ip,
                    dev.latency_ms,
                    network_id,
                ).await {
                    let _ = conn.execute("ROLLBACK", ()).await;
                    return Err(format!("history record failed: {e}"));
                }

                if is_new {
                    // Record event
                    if let Err(e) = super::history::record_new_device_event(
                        &conn,
                        device_id,
                        &dev.ip,
                        &dev.mac,
                        if dev.vendor.is_empty() { None } else { Some(&dev.vendor) },
                        now,
                    ).await {
                        let _ = conn.execute("ROLLBACK", ()).await;
                        return Err(format!("event record failed: {e}"));
                    }

                    new_devices.push((
                        dev.name.clone(),
                        dev.vendor.clone(),
                        dev.ip.clone(),
                        dev.mac.clone(),
                    ));
                }
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                return Err(format!("device upsert failed: {e}"));
            }
        }
    }
    
    // Mark offline
    let seen_macs: Vec<String> = devices.iter().map(|d| d.mac.clone()).collect();
    if let Err(e) = mark_offline_except(&conn, &seen_macs).await {
        let _ = conn.execute("ROLLBACK", ()).await;
        return Err(format!("mark offline failed: {e}"));
    }
    
    conn.execute("COMMIT", ()).await.map_err(|e| e.to_string())?;
    
    Ok(new_devices)
}

/// List all devices asynchronously, optionally filtering for online only.
pub async fn list_devices_async(db: AppDb, online_only: bool) -> Result<Vec<DeviceRecord>, String> {
    let conn = db.connect_dedicated()?;
    list_devices(&conn, online_only).await
}

/// Get a device by its MAC address asynchronously.
pub async fn get_device_by_mac_async(db: AppDb, mac: String) -> Result<Option<DeviceRecord>, String> {
    let conn = db.connect_dedicated()?;
    get_device_by_mac(&conn, &mac).await
}

/// Upsert a device alias asynchronously.
pub async fn upsert_device_alias(
    db: AppDb,
    ip_address: String,
    alias_name: String,
) -> Result<(), String> {
    let conn = db.connect_dedicated()?;
    conn.execute(
        "INSERT INTO device_aliases (ip_address, alias_name) 
         VALUES (?1, ?2) 
         ON CONFLICT(ip_address) DO UPDATE SET alias_name = excluded.alias_name",
        params![ip_address, alias_name],
    )
    .await
    .map_err(|e| format!("upsert device alias: {e}"))?;
    Ok(())
}

/// Update custom fields for a device asynchronously.
pub async fn update_device_custom_fields(
    db: AppDb,
    mac: String,
    custom_name: Option<String>,
    notes: Option<String>,
    acknowledged: Option<bool>,
    custom_icon: Option<String>,
) -> Result<(), String> {
    let conn = db.connect_dedicated()?;
    conn.execute(
        "UPDATE devices SET 
            custom_name = COALESCE(?2, custom_name), 
            notes = COALESCE(?3, notes), 
            acknowledged = COALESCE(?4, acknowledged),
            custom_icon = COALESCE(?5, custom_icon)
         WHERE mac = ?1",
        params![
            mac,
            custom_name,
            notes,
            acknowledged.map(|b| b as i64),
            custom_icon,
        ],
    )
    .await
    .map_err(|e| format!("update device custom fields: {e}"))?;
    Ok(())
}

/// Upsert a discovered device. Returns `(device_id, is_new)`.
pub async fn upsert_discovered_device(
    conn: &Connection,
    dev: &DiscoveredDevice,
    now_ms: i64,
    network_id: Option<i64>,
) -> Result<(i64, bool), String> {
    // Use ON CONFLICT (mac) DO UPDATE for a single-pass upsert
    let sql = "
        INSERT INTO devices (
            mac, first_seen, last_seen, last_ip, vendor, 
            likely_type, hostname, mdns_hostname, ssdp_server, 
            display_name, network_id, is_online, is_active
        )
        VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, 1)
        ON CONFLICT(mac) DO UPDATE SET
            last_seen     = excluded.last_seen,
            last_ip       = excluded.last_ip,
            vendor        = COALESCE(excluded.vendor, devices.vendor),
            likely_type   = COALESCE(excluded.likely_type, devices.likely_type),
            hostname      = COALESCE(excluded.hostname, devices.hostname),
            mdns_hostname = COALESCE(excluded.mdns_hostname, devices.mdns_hostname),
            ssdp_server   = COALESCE(excluded.ssdp_server, devices.ssdp_server),
            network_id    = COALESCE(excluded.network_id, devices.network_id),
            is_online     = 1,
            is_active     = 1
    ";

    let res = conn.execute(sql, params![
        dev.mac.clone(),
        now_ms,
        dev.ip.clone(),
        nonempty(&dev.vendor),
        dev.likely_type.as_deref(),
        dev.hostname.as_deref(),
        dev.mdns_hostname.as_deref(),
        dev.ssdp_server.as_deref(),
        nonempty(&dev.name),
        network_id,
    ]).await;

    match res {
        Ok(rows_affected) => {
            let is_new = rows_affected == 1;
            let mut rows = conn
                .query(
                    "SELECT id FROM devices WHERE mac = ?1",
                    params![dev.mac.clone()],
                )
                .await
                .map_err(|e| format!("device id lookup: {e}"))?;

            if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
                let id: i64 = row.get::<Option<i64>>(0).map_err(|e| e.to_string())?.unwrap_or_default();
                Ok((id, is_new))
            } else {
                Err("device id not found after upsert".to_string())
            }
        }
        Err(e) => {
            log::error!("[DB] Upsert failed for MAC {}: {}", dev.mac, e);
            Err(format!("device upsert error: {e}"))
        }
    }
}

pub async fn mark_offline_except(conn: &Connection, seen_macs: &[String]) -> Result<usize, String> {
    let n = if seen_macs.is_empty() {
        conn.execute("UPDATE devices SET is_online = 0 WHERE is_online = 1", ())
            .await
            .map_err(|e| format!("mark_offline_all: {e}"))?
    } else {
        let placeholders = seen_macs.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE devices SET is_online = 0 WHERE is_online = 1 AND mac NOT IN ({placeholders})"
        );
        // libsql::params_from_iter equivalent
        let mut params_vec = Vec::new();
        for mac in seen_macs {
            params_vec.push(libsql::Value::from(mac.clone()));
        }
        conn.execute(&sql, params_vec)
            .await
            .map_err(|e| format!("mark_offline_except: {e}"))?
    };
    Ok(n as usize)
}

async fn backfill_likely_type(conn: &Connection) {
    const GENERIC: &[&str] = &["", "Network Device", "Router / Gateway"];

    let mut rows = match conn.query(
        "SELECT mac, vendor, hostname, mdns_hostname
         FROM devices
         WHERE likely_type IS NULL
            OR likely_type = ''
            OR likely_type = 'Network Device'
            OR likely_type = 'Router / Gateway'",
        (),
    ).await {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut candidates = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        candidates.push((
            row.get::<Option<String>>(0).unwrap_or_default().unwrap_or_default(),
            row.get::<Option<String>>(1).unwrap_or_default(),
            row.get::<Option<String>>(2).unwrap_or_default(),
            row.get::<Option<String>>(3).unwrap_or_default(),
        ));
    }

    for (mac, vendor, hostname, mdns_hostname) in candidates {
        let new_type = [mdns_hostname.as_deref(), hostname.as_deref()]
            .into_iter()
            .flatten()
            .find_map(classify_from_hostname)
            .or_else(|| {
                vendor
                    .as_deref()
                    .and_then(|v| classify_from_vendor(v, &[]))
            });

        if let Some(t) = new_type {
            if GENERIC.contains(&t.as_str()) {
                continue;
            }
            let _ = conn.execute(
                "UPDATE devices SET likely_type = ?1
                  WHERE mac = ?2
                    AND (likely_type IS NULL
                         OR likely_type = ''
                         OR likely_type = 'Network Device'
                         OR likely_type = 'Router / Gateway')",
                params![t, mac],
            ).await;
        }
    }
}

pub async fn list_devices(conn: &Connection, online_only: bool) -> Result<Vec<DeviceRecord>, String> {
    backfill_likely_type(conn).await;
    let sql = if online_only {
        "SELECT d.id, d.mac, d.first_seen, d.last_seen, d.last_ip, d.vendor, d.custom_name,
                d.likely_type, d.hostname, d.mdns_hostname, d.ssdp_server, d.interrogation_name,
                d.acknowledged, d.notes, COALESCE(a.alias_name, d.display_name), d.is_online, d.custom_icon
         FROM devices d
         LEFT JOIN device_aliases a ON d.last_ip = a.ip_address
         WHERE d.is_online = 1 ORDER BY d.last_seen DESC"
    } else {
        "SELECT d.id, d.mac, d.first_seen, d.last_seen, d.last_ip, d.vendor, d.custom_name,
                d.likely_type, d.hostname, d.mdns_hostname, d.ssdp_server, d.interrogation_name,
                d.acknowledged, d.notes, COALESCE(a.alias_name, d.display_name), d.is_online, d.custom_icon
         FROM devices d
         LEFT JOIN device_aliases a ON d.last_ip = a.ip_address
         ORDER BY d.last_seen DESC"
    };
    
    let mut rows = conn
        .query(sql, ())
        .await
        .map_err(|e| format!("query list: {e}"))?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        results.push(row_to_device(&row).map_err(|e| e.to_string())?);
    }

    Ok(results)
}

pub async fn get_device_by_mac(conn: &Connection, mac: &str) -> Result<Option<DeviceRecord>, String> {
    let mut rows = conn.query(
        "SELECT d.id, d.mac, d.first_seen, d.last_seen, d.last_ip, d.vendor, d.custom_name,
                d.likely_type, d.hostname, d.mdns_hostname, d.ssdp_server, d.interrogation_name,
                d.acknowledged, d.notes, COALESCE(a.alias_name, d.display_name), d.is_online, d.custom_icon
         FROM devices d
         LEFT JOIN device_aliases a ON d.last_ip = a.ip_address
         WHERE d.mac = ?1",
        params![mac],
    ).await.map_err(|e| format!("get device: {e}"))?;

    if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        Ok(Some(row_to_device(&row).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

pub async fn delete_device(conn: &Connection, mac: &str) -> Result<bool, String> {
    let changed = conn
        .execute("DELETE FROM devices WHERE mac = ?1", params![mac])
        .await
        .map_err(|e| format!("delete device: {e}"))?;
    Ok(changed == 1)
}

fn row_to_device(row: &Row) -> Result<DeviceRecord, libsql::Error> {
    Ok(DeviceRecord {
        id:                 row.get::<Option<i64>>(0)?.unwrap_or_default(),
        mac:                row.get::<Option<String>>(1)?.unwrap_or_default(),
        first_seen:         row.get::<Option<i64>>(2)?.unwrap_or_default(),
        last_seen:          row.get::<Option<i64>>(3)?.unwrap_or_default(),
        last_ip:            row.get::<Option<String>>(4)?,
        vendor:             row.get::<Option<String>>(5)?,
        custom_name:        row.get::<Option<String>>(6)?,
        likely_type:        row.get::<Option<String>>(7)?,
        hostname:           row.get::<Option<String>>(8)?,
        mdns_hostname:      row.get::<Option<String>>(9)?,
        ssdp_server:        row.get::<Option<String>>(10)?,
        interrogation_name: row.get::<Option<String>>(11)?,
        acknowledged:       row.get::<Option<i64>>(12)?.unwrap_or_default() != 0,
        notes:              row.get::<Option<String>>(13)?,
        display_name:       row.get::<Option<String>>(14)?,
        is_online:          row.get::<Option<i64>>(15)?.unwrap_or_default() != 0,
        custom_icon:        row.get::<Option<String>>(16)?,
    })
}

fn nonempty(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}
