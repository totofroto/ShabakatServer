CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS networks (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    ssid       TEXT,
    bssid      TEXT UNIQUE NOT NULL,
    gateway    TEXT,
    subnet     TEXT,
    first_seen INTEGER NOT NULL,
    last_seen  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS devices (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    mac                TEXT UNIQUE NOT NULL,
    first_seen         INTEGER NOT NULL,
    last_seen          INTEGER NOT NULL,
    last_ip            TEXT,
    vendor             TEXT,
    custom_name        TEXT,
    likely_type        TEXT,
    hostname           TEXT,
    mdns_hostname      TEXT,
    ssdp_server        TEXT,
    interrogation_name TEXT,
    acknowledged       INTEGER NOT NULL DEFAULT 0,
    notes              TEXT,
    display_name       TEXT,
    is_online          INTEGER NOT NULL DEFAULT 0,
    is_active          INTEGER NOT NULL DEFAULT 1,
    network_id         INTEGER REFERENCES networks(id)
);

CREATE INDEX IF NOT EXISTS idx_devices_active ON devices(is_active);

CREATE TABLE IF NOT EXISTS scan_history (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id    TEXT    NOT NULL,
    scanned_at INTEGER NOT NULL,
    device_id  INTEGER NOT NULL REFERENCES devices(id),
    ip         TEXT    NOT NULL,
    is_online  INTEGER NOT NULL,
    latency_ms REAL,
    open_ports TEXT,
    network_id INTEGER REFERENCES networks(id)
);

CREATE INDEX IF NOT EXISTS idx_scan_history_device ON scan_history(device_id, scanned_at);
CREATE INDEX IF NOT EXISTS idx_scan_history_time   ON scan_history(scanned_at);

CREATE TABLE IF NOT EXISTS device_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT    NOT NULL,
    device_id  INTEGER REFERENCES devices(id),
    mac        TEXT,
    timestamp  INTEGER NOT NULL,
    details    TEXT
);

CREATE INDEX IF NOT EXISTS idx_device_events_time ON device_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_device_events_mac  ON device_events(mac);

CREATE TABLE IF NOT EXISTS outages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER,
    duration_ms INTEGER
);

CREATE TABLE IF NOT EXISTS speed_tests (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    tested_at     INTEGER NOT NULL,
    download_mbps REAL,
    upload_mbps   REAL,
    ping_ms       REAL
);

CREATE TABLE IF NOT EXISTS dns_providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    ip TEXT NOT NULL,
    port INTEGER NOT NULL,
    username TEXT,
    password TEXT,
    is_enabled INTEGER DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS device_aliases (
    ip_address TEXT PRIMARY KEY,
    alias_name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS system_status (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    score             INTEGER NOT NULL,
    performance_score INTEGER NOT NULL,
    latency_score     INTEGER NOT NULL,
    security_score    INTEGER NOT NULL,
    last_updated      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS notification_providers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    enabled INTEGER DEFAULT 0,
    config_json TEXT NOT NULL
);
