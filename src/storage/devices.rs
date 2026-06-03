use crate::scanner::arp::normalize_mac;
use crate::scanner::fingerprints::{classify_from_hostname, classify_from_vendor};
use crate::storage::AppDb;
use crate::types::{DiscoveredDevice, DeviceRecord};
use rusqlite::{params, Connection, Row};

pub struct DeviceStorage;

impl DeviceStorage {
    pub async fn get_all_devices(db: &AppDb) -> Result<Vec<DeviceRecord>, String> {
        list_devices_async(db.clone(), false).await
    }
}

pub async fn complete_scan_persistence(
    db: AppDb,
    devices: Vec<DiscoveredDevice>,
    scan_id: String,
    network_id: Option<i64>,
) -> Result<Vec<(String, String, String, String)>, String> {
    let now = super::now_ms();

    db.execute(move |conn| {
        if let Err(e) = conn.execute("PRAGMA busy_timeout = 5000", []) {
            log::warn!("[DB] busy_timeout pragma failed (non-fatal): {e}");
        }

        let tx = conn.unchecked_transaction().map_err(|e| {
            log::error!("[DB] [FLIGHT_RECORDER] Transaction start failed: {e}");
            e.to_string()
        })?;

        let mut new_devices = Vec::new();

        for dev in &devices {
            match upsert_discovered_device(&tx, dev, now, network_id) {
                Ok((device_id, is_new, is_ignored)) => {
                    if let Err(e) = super::history::record_device_online(
                        &tx,
                        super::history::OnlineRecord {
                            scan_id: &scan_id,
                            scanned_at: now,
                            device_id,
                            ip: &dev.ip,
                            latency_ms: dev.latency_ms,
                            network_id,
                            rssi: dev.rssi,
                        },
                    ) {
                        log::error!(
                            "[DB] [FLIGHT_RECORDER] record_device_online failed for IP {} (device_id={device_id}): {e}",
                            dev.ip
                        );
                        return Err(format!("history record failed: {e}"));
                    }

                    if is_new && !is_ignored {
                        if let Err(e) = super::history::record_new_device_event(
                            &tx,
                            device_id,
                            &dev.ip,
                            &dev.mac,
                            if dev.vendor.is_empty() { None } else { Some(&dev.vendor) },
                            now,
                        ) {
                            log::error!(
                                "[DB] [FLIGHT_RECORDER] record_new_device_event failed for MAC {}: {e}",
                                dev.mac
                            );
                        }
                        
                        // Use display_name if available, otherwise fallback to whatever we have
                        let name = dev.hostname.clone()
                            .or(dev.mdns_hostname.clone())
                            .or(dev.ssdp_server.clone())
                            .unwrap_or_else(|| dev.ip.clone());

                        new_devices.push((name, dev.vendor.clone(), dev.ip.clone(), dev.mac.clone()));
                    }
                }
                Err(e) => {
                    log::error!(
                        "[DB] [FLIGHT_RECORDER] upsert_discovered_device failed for MAC {}: {e}",
                        dev.mac
                    );
                    return Err(format!("upsert failed: {e}"));
                }
            }
        }

        tx.commit().map_err(|e| {
            log::error!("[DB] [FLIGHT_RECORDER] Transaction commit failed: {e}");
            e.to_string()
        })?;
        Ok(new_devices)
    })
    .await
}

pub async fn list_devices_async(db: AppDb, online_only: bool) -> Result<Vec<DeviceRecord>, String> {
    db.execute(move |conn| list_devices(conn, online_only)).await
}

pub async fn get_device_by_mac_async(db: AppDb, mac: String) -> Result<Option<DeviceRecord>, String> {
    db.execute(move |conn| get_device_by_mac(conn, &mac)).await
}

pub async fn upsert_device_alias(
    db: AppDb,
    ip_address: String,
    alias_name: String,
) -> Result<(), String> {
    db.execute(move |conn| -> Result<(), String> {
        conn.execute(
            "INSERT INTO device_aliases (ip_address, alias_name) 
             VALUES (?1, ?2) 
             ON CONFLICT(ip_address) DO UPDATE SET alias_name = excluded.alias_name",
            params![ip_address, alias_name],
        )
        .map_err(|e| format!("upsert device alias: {e}"))?;
        Ok(())
    }).await
}

pub async fn update_device_custom_fields(
    db: AppDb,
    mac: String,
    custom_name: Option<String>,
    notes: Option<String>,
    acknowledged: Option<bool>,
    custom_icon: Option<String>,
) -> Result<(), String> {
    db.execute(move |conn| {
        patch_device(conn, &mac, custom_name, notes, acknowledged, custom_icon).map(|_| ())
    }).await
}

pub async fn ignore_device_async(db: AppDb, mac: String) -> Result<(), String> {
    db.execute(move |conn| -> Result<(), String> {
        conn.execute(
            "UPDATE devices SET is_ignored = 1 WHERE mac = ?1",
            params![mac],
        ).map_err(|e| format!("ignore device: {e}"))?;
        Ok(())
    }).await
}

pub fn list_devices(conn: &Connection, online_only: bool) -> Result<Vec<DeviceRecord>, String> {
    backfill_likely_type(conn);
    let sql = if online_only {
        "SELECT d.id, d.mac, d.first_seen, d.last_seen, d.last_ip, d.vendor, d.custom_name,
                d.likely_type, d.hostname, d.mdns_hostname, d.ssdp_server, d.interrogation_name,
                d.acknowledged, d.notes, COALESCE(a.alias_name, d.display_name), d.is_online, d.is_ignored, d.rssi, d.custom_icon
         FROM devices d
         LEFT JOIN device_aliases a ON d.last_ip = a.ip_address
         WHERE (strftime('%s','now') * 1000 - d.last_seen) < 900000 
         ORDER BY d.last_seen DESC"
    } else {
        "SELECT d.id, d.mac, d.first_seen, d.last_seen, d.last_ip, d.vendor, d.custom_name,
                d.likely_type, d.hostname, d.mdns_hostname, d.ssdp_server, d.interrogation_name,
                d.acknowledged, d.notes, COALESCE(a.alias_name, d.display_name), d.is_online, d.is_ignored, d.rssi, d.custom_icon
         FROM devices d
         LEFT JOIN device_aliases a ON d.last_ip = a.ip_address
         ORDER BY d.last_seen DESC"
    };
    
    let mut stmt = conn.prepare(sql).map_err(|e| format!("prepare list devices: {e}"))?;
    let rows = stmt
        .query_map([], row_to_device)
        .map_err(|e| format!("query list devices: {e}"))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| format!("map list devices: {e}"))?);
    }
    Ok(results)
}

pub fn get_device_by_mac(conn: &Connection, mac: &str) -> Result<Option<DeviceRecord>, String> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.mac, d.first_seen, d.last_seen, d.last_ip, d.vendor, d.custom_name,
                d.likely_type, d.hostname, d.mdns_hostname, d.ssdp_server, d.interrogation_name,
                d.acknowledged, d.notes, COALESCE(a.alias_name, d.display_name), d.is_online, d.is_ignored, d.rssi, d.custom_icon
         FROM devices d
         LEFT JOIN device_aliases a ON d.last_ip = a.ip_address
         WHERE d.mac = ?1",
    ).map_err(|e| format!("prepare get device: {e}"))?;

    let mut rows = stmt
        .query_map(params![mac], row_to_device)
        .map_err(|e| format!("query get device: {e}"))?;

    if let Some(row) = rows.next() {
        return Ok(Some(row.map_err(|e| format!("map get device: {e}"))?));
    }
    Ok(None)
}

pub fn patch_device(
    conn: &Connection,
    mac: &str,
    custom_name: Option<String>,
    notes: Option<String>,
    acknowledged: Option<bool>,
    custom_icon: Option<String>,
) -> Result<bool, String> {
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(name) = custom_name {
        updates.push("custom_name = ?");
        params.push(Box::new(nonempty(&name).map(|s| s.to_string())));
    }
    if let Some(n) = notes {
        updates.push("notes = ?");
        params.push(Box::new(nonempty(&n).map(|s| s.to_string())));
    }
    if let Some(ack) = acknowledged {
        updates.push("acknowledged = ?");
        params.push(Box::new(if ack { 1 } else { 0 }));
    }
    if let Some(icon) = custom_icon {
        updates.push("custom_icon = ?");
        params.push(Box::new(nonempty(&icon).map(|s| s.to_string())));
    }

    if updates.is_empty() {
        return Ok(true);
    }

    let sql = format!(
        "UPDATE devices SET {} WHERE mac = ?",
        updates.join(", ")
    );
    
    // Add mac as the last parameter
    params.push(Box::new(mac.to_string()));

    // Convert Box<dyn ToSql> to &dyn ToSql
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = conn.execute(&sql, &params_refs[..])
        .map_err(|e| format!("update device: {e}"))?;
    
    Ok(rows > 0)
}

pub fn delete_device(conn: &Connection, mac: &str) -> Result<bool, String> {
    let rows = conn.execute("DELETE FROM devices WHERE mac = ?1", params![mac])
        .map_err(|e| format!("delete device: {e}"))?;
    Ok(rows > 0)
}

fn upsert_discovered_device(
    conn: &Connection,
    dev: &DiscoveredDevice,
    now_ms: i64,
    network_id: Option<i64>,
) -> Result<(i64, bool, bool), String> {
    let vendor = nonempty(&dev.vendor_name).or(nonempty(&dev.vendor));
    
    // Fix: classify_from_vendor takes 3 arguments.
    let current_label = ""; 
    let open_ports: Vec<u16> = dev.open_ports.as_deref()
        .map(|s| s.split(',').filter_map(|p| p.parse().ok()).collect())
        .unwrap_or_default();

    let likely_type = dev.likely_type.as_deref()
        .map(|s| s.to_string())
        .or_else(|| classify_from_hostname(dev.hostname.as_deref().unwrap_or_default()).map(|s| s.to_string()))
        .or_else(|| classify_from_vendor(vendor.unwrap_or_default(), &open_ports, current_label).map(|s| s.to_string()));

    let display_name = dev.hostname.as_deref()
        .or(dev.mdns_hostname.as_deref())
        .or(dev.ssdp_server.as_deref())
        .or(nonempty(&dev.name));

    // Use ON CONFLICT (mac) DO UPDATE for a single-pass upsert
    let sql = "
        INSERT INTO devices (
            mac, first_seen, last_seen, last_ip, vendor, 
            likely_type, hostname, mdns_hostname, ssdp_server, 
            display_name, network_id, is_online, is_active, rssi
        )
        VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, 1, ?11)
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
            is_active     = 1,
            rssi          = excluded.rssi
    ";

    let res = conn.execute(sql, params![
        normalize_mac(&dev.mac),
        now_ms,
        dev.ip.clone(),
        vendor,
        likely_type,
        dev.hostname.as_deref(),
        dev.mdns_hostname.as_deref(),
        dev.ssdp_server.as_deref(),
        display_name,
        network_id,
        dev.rssi,
    ]);

    match res {
        Ok(rows_affected) => {
            let is_new = rows_affected == 1;
            let row: (i64, i64) = conn.query_row(
                "SELECT id, is_ignored FROM devices WHERE mac = ?1",
                params![dev.mac],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ).map_err(|e| format!("fetch upserted id: {e}"))?;
            Ok((row.0, is_new, row.1 != 0))
        }
        Err(e) => Err(format!("upsert device: {e}")),
    }
}

fn backfill_likely_type(conn: &Connection) {
    let _ = conn.execute(
        "UPDATE devices SET likely_type = 'Router' WHERE likely_type IS NULL AND (hostname LIKE '%router%' OR hostname LIKE '%gateway%')",
        [],
    );
}

fn row_to_device(row: &Row) -> rusqlite::Result<DeviceRecord> {
    Ok(DeviceRecord {
        id:                 row.get(0)?,
        mac:                row.get(1)?,
        first_seen:         row.get::<_, Option<i64>>(2)?.unwrap_or_default(),
        last_seen:          row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
        last_ip:            row.get(4)?,
        vendor:             row.get(5)?,
        custom_name:        row.get(6)?,
        likely_type:        row.get(7)?,
        hostname:           row.get(8)?,
        mdns_hostname:      row.get(9)?,
        ssdp_server:        row.get(10)?,
        interrogation_name: row.get(11)?,
        acknowledged:       row.get::<_, Option<i64>>(12)?.unwrap_or_default() != 0,
        notes:              row.get(13)?,
        display_name:       row.get(14)?,
        is_online:          row.get::<_, Option<i64>>(15)?.unwrap_or_default() != 0,
        is_ignored:         row.get::<_, Option<i64>>(16)?.unwrap_or_default() != 0,
        rssi:               row.get::<_, Option<i32>>(17)?,
        custom_icon:        row.get::<_, Option<String>>(18)?,
        suggested_names:    None,
    })
}

fn nonempty(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}
