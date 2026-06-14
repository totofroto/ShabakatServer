-- =============================================================================
-- Shabakat Server — SQLite Schema
-- =============================================================================
-- Design rules:
--   • All CREATE TABLE / CREATE INDEX statements use IF NOT EXISTS so the
--     entire file can be re-applied idempotently on every startup without
--     errors or data loss.
--   • PRAGMA foreign_keys = ON is a session-level setting; it is enforced
--     in Rust connection initialisation code, NOT here.
--   • All timestamps are stored as INTEGER (Unix epoch, milliseconds).
--   • JSON payloads are stored as TEXT; SQLite's json() helpers can be used
--     in queries when needed.
-- =============================================================================


-- ---------------------------------------------------------------------------
-- TABLE: devices
-- Core device registry — one row per unique physical device on the LAN.
-- The MAC address is the stable primary key for all cross-table references.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS devices (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Layer 2 identity (normalised to uppercase XX:XX:XX:XX:XX:XX)
    mac                 TEXT    UNIQUE NOT NULL,

    -- Temporal bookkeeping (Unix ms)
    first_seen          INTEGER NOT NULL,
    last_seen           INTEGER NOT NULL,

    -- Network-layer observations
    last_ip             TEXT,           -- Most-recently observed IPv4 address
    hostname            TEXT,           -- Reverse-DNS PTR record result
    mdns_hostname       TEXT,           -- Zeroconf / mDNS .local hostname

    -- Passive enrichment from ambient multicast (Digital Fence)
    ssdp_server         TEXT,           -- UPnP / SSDP "SERVER:" header banner
    interrogation_name  TEXT,           -- HTTP <title> or UPnP friendly-name

    -- OUI / fingerprint intelligence
    vendor              TEXT,           -- IEEE OUI manufacturer string
    likely_type         TEXT,           -- Fingerprint label ("Router", "NAS", …)

    -- User-space annotations
    custom_name         TEXT,           -- User-assigned display label
    acknowledged        INTEGER NOT NULL DEFAULT 0, -- 1 = user has marked as known
    notes               TEXT,           -- Free-form markdown annotation
    is_active           INTEGER NOT NULL DEFAULT 1

    -- CHECK constraint ensures the acknowledged flag stays boolean
    , CHECK (acknowledged IN (0, 1))
);

-- Primary lookup: every scan merge does a WHERE mac = ?
CREATE UNIQUE INDEX IF NOT EXISTS idx_devices_mac
    ON devices (mac);

-- Dashboard default sort: most-recently-seen devices first
CREATE INDEX IF NOT EXISTS idx_devices_last_seen
    ON devices (last_seen DESC);

-- Alert dashboard: quickly surface all unacknowledged / rogue devices
CREATE INDEX IF NOT EXISTS idx_devices_unacked
    ON devices (acknowledged, last_seen DESC)
    WHERE acknowledged = 0;


-- ---------------------------------------------------------------------------
-- TABLE: scan_history
-- Append-only per-scan-per-device timeline snapshots.
-- NEVER UPDATE or DELETE rows — the timeline is an audit log.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS scan_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Which scan run produced this row
    -- Format: "scheduled-{n}" | "manual-{n}" | "heartbeat-{n}"
    scan_id     TEXT    NOT NULL,

    -- Wall-clock time the probe was executed (Unix ms)
    scanned_at  INTEGER NOT NULL,

    -- The device this row describes
    device_id   INTEGER NOT NULL
                    REFERENCES devices (id)
                    ON DELETE CASCADE,

    -- Observed network state at probe time
    ip          TEXT    NOT NULL,
    is_online   INTEGER NOT NULL CHECK (is_online IN (0, 1)),
    latency_ms  REAL,           -- NULL = device did not respond (timeout / offline)

    -- Port Guardian snapshot; JSON array of integers, e.g. [22, 80, 443]
    open_ports  TEXT
);

-- Per-device history queries: "show me when this device was online this week"
CREATE INDEX IF NOT EXISTS idx_scan_history_device_time
    ON scan_history (device_id, scanned_at DESC);

-- Global timeline endpoint: GET /api/history?from=&to=
CREATE INDEX IF NOT EXISTS idx_scan_history_time
    ON scan_history (scanned_at DESC);

-- Scan-run replay: fetch all devices seen in a specific run
CREATE INDEX IF NOT EXISTS idx_scan_history_scan_id
    ON scan_history (scan_id, scanned_at DESC);


-- ---------------------------------------------------------------------------
-- TABLE: device_events
-- Lifecycle transitions, security alerts, and Digital Fence triggers.
-- Each row is immutable once inserted — this is an event ledger.
--
-- Event type vocabulary:
--   "new_device"      — first time this MAC has ever been observed
--   "device_online"   — returned online after one or more missed scans
--   "device_offline"  — missed N consecutive scheduled probes
--   "port_change"     — the set of open ports changed vs previous scan
--   "intruder_alert"  — unacknowledged device appeared on the network
--   "fence_trigger"   — passive mDNS/SSDP Digital Fence detected ambient traffic
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS device_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Discriminant; use one of the constants listed above
    event_type  TEXT    NOT NULL,

    -- NULL if the device record has since been deleted (ON DELETE SET NULL)
    device_id   INTEGER REFERENCES devices (id) ON DELETE SET NULL,

    -- When the event was detected (Unix ms)
    timestamp   INTEGER NOT NULL,

    -- Event-type-specific JSON payload (varies per event_type)
    -- Examples:
    --   new_device:     {"ip":"192.168.1.50","vendor":"Apple"}
    --   port_change:    {"added":[8080],"removed":[22]}
    --   intruder_alert: {"ip":"192.168.1.99","mac":"DE:AD:BE:EF:CA:FE"}
    details     TEXT
);

-- Per-device activity feed (Device Details panel → Activity tab)
CREATE INDEX IF NOT EXISTS idx_device_events_device_time
    ON device_events (device_id, timestamp DESC);

-- Global event feed: GET /api/events?limit=50
CREATE INDEX IF NOT EXISTS idx_device_events_time
    ON device_events (timestamp DESC);

-- Filtered event views: show only intruder_alert rows, etc.
CREATE INDEX IF NOT EXISTS idx_device_events_type_time
    ON device_events (event_type, timestamp DESC);


-- ---------------------------------------------------------------------------
-- TABLE: settings
-- Simple key → value pairs for server runtime configuration.
-- All values are stored as TEXT; the application layer interprets types.
--
-- Seed keys (inserted lazily by the application):
--   "scan_interval_secs"      — seconds between scheduled full scans
--   "heartbeat_interval_secs" — seconds between heartbeat pings
--   "subnet_override"         — manual CIDR or "auto"
--   "notifications_enabled"   — "true" | "false"
--   "schema_version"          — mirrors PRAGMA user_version for visibility
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
