use libsql::Connection;

const SCHEMA: &str = include_str!("schema.sql");

pub async fn run(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(SCHEMA)
        .await
        .map_err(|e| format!("migration failed: {e}"))?;
    // Add new columns to existing databases (SQLite does not support IF NOT EXISTS on ALTER TABLE).
    let _ = conn.execute("ALTER TABLE devices ADD COLUMN display_name TEXT", ()).await;
    let _ = conn.execute(
        "ALTER TABLE devices ADD COLUMN is_online INTEGER NOT NULL DEFAULT 0",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE devices ADD COLUMN network_id INTEGER REFERENCES networks(id)",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE devices ADD COLUMN custom_icon TEXT",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE scan_history ADD COLUMN network_id INTEGER REFERENCES networks(id)",
        (),
    ).await;
    // Clear display_name rows that were auto-set to the same value as hostname —
    // they are redundant and will shadow live hostname changes on rescan.
    let _ = conn.execute(
        "UPDATE devices SET display_name = NULL WHERE display_name IS NOT NULL AND display_name = hostname",
        (),
    ).await;
    // Clear stale ghost entry: id=1 has mac="Unknown" with mixed-up data
    // (Yamaha SSDP banner on WADDAN's IP). Wipe its metadata so next scan
    // re-discovers it cleanly.
    let _ = conn.execute(
        "UPDATE devices SET display_name=NULL, hostname=NULL, mdns_hostname=NULL,
         ssdp_server=NULL, likely_type=NULL, vendor=NULL
         WHERE mac='Unknown' AND id=1",
        (),
    ).await;
    // New tables added in Sprint 1 — CREATE IF NOT EXISTS is idempotent.
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS outages (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at  INTEGER NOT NULL,
            ended_at    INTEGER,
            duration_ms INTEGER
        )",
        (),
    ).await;
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS speed_tests (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            tested_at     INTEGER NOT NULL,
            download_mbps REAL,
            upload_mbps   REAL,
            ping_ms       REAL
        )",
        (),
    ).await;
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS dns_providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            ip TEXT NOT NULL,
            port INTEGER NOT NULL,
            username TEXT,
            password TEXT,
            is_enabled INTEGER DEFAULT 1,
            created_at INTEGER NOT NULL
        )",
        (),
    ).await;
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS device_aliases (
            ip_address TEXT PRIMARY KEY,
            alias_name TEXT NOT NULL
        )",
        (),
    ).await;
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS system_status (
            id                INTEGER PRIMARY KEY CHECK (id = 1),
            score             INTEGER NOT NULL,
            performance_score INTEGER NOT NULL,
            latency_score     INTEGER NOT NULL,
            security_score    INTEGER NOT NULL,
            last_updated      INTEGER NOT NULL
        )",
        (),
    ).await;
    Ok(())
}
