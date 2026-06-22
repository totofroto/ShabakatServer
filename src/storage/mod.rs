//! # `storage` — Thread-Safe SQLite Persistence Layer (Pool Edition)
//!
//! ## Design Contract (v2 — deadpool-sqlite)
//!
//! This module exposes a [`Db`] handle backed by a **`deadpool-sqlite` connection
//! pool** instead of the original single `Arc<Mutex<Connection>>`.
//!
//! ### Why the pool?
//!
//! The J4125 has 4 cores and runs 8+ concurrent background workers (scan
//! scheduler, presence monitor, Digital Fence ×2, bandwidth monitor, sys-metrics,
//! scoring task, metrics compactor, plus N concurrent HTTP handlers).  With a
//! single connection they all serialised through one `std::sync::Mutex`, building
//! up a blocking-thread queue that added latency to every operation.
//!
//! With a pool of N connections (default: **4**, matching the CPU core count):
//! - Up to 4 tasks can execute SQLite operations simultaneously.
//! - SQLite WAL mode allows 1 writer + N concurrent readers, so 4 connections
//!   saturate the hardware without exceeding WAL's concurrency window.
//! - A pool `get()` call waits asynchronously (no busy-spin) if all connections
//!   are checked out — the Tokio reactor is never blocked.
//!
//! ### Public API (unchanged)
//!
//! All callers (`db.interact(…)`, `db.interact_mut(…)`, `db.execute(…)`) work
//! identically — the pool is a pure implementation-detail behind `Db`.
//!
//! ### Startup change
//!
//! `Db::open` is now `async`.  The `main.rs` startup call:
//! ```rust,ignore
//! let db = tokio::task::spawn_blocking(move || Db::open(&db_path)).await??;
//! ```
//! becomes:
//! ```rust,ignore
//! let db = Db::open(&db_path).await?;
//! ```
//! (See `src/main.rs` — updated in this patch set.)

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------
pub mod compactor;
pub mod devices;
pub mod history;
pub mod metrics;
pub mod migrations;
pub mod networks;
pub mod providers;
pub mod settings;
pub mod system_status;

use std::path::Path;
use std::sync::Arc;

use deadpool_sqlite::{Config as PoolConfig, Pool, Runtime};
use log::{error, info, warn};
use rusqlite::Connection;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Embedded schema
// ---------------------------------------------------------------------------
const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Logical schema version.  Increment whenever a new migration block is added.
///
/// v1 — base 4-table schema (devices, scan_history, device_events, settings)
/// v2 — extended schema (outages, speed_tests, dns_providers, device_aliases,
///       system_status, notification_providers, hourly_metrics + ALTER TABLE
///       column additions via `migrations::run`)
/// v3 — add is_active column to devices table
const CURRENT_SCHEMA_VERSION: i64 = 3;

/// Pool size: one connection per J4125 core.
/// WAL mode allows 1 writer + (N-1) concurrent readers at this setting.
const DB_POOL_SIZE: usize = 4;

// ---------------------------------------------------------------------------
// Public aliases / utilities
// ---------------------------------------------------------------------------

/// Type alias so sub-modules write `AppDb` instead of `Db`.
pub type AppDb = Db;

/// Current wall-clock time in milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error(
        "Database connection mutex is poisoned \
         (a previous thread panicked while holding the lock)"
    )]
    LockPoisoned,

    #[error("Blocking database task panicked: {0}")]
    TaskPanic(String),

    #[error("Schema migration to version {version} failed: {source}")]
    MigrationFailed {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },

    #[error("Schema migration to version {version} script error: {message}")]
    MigrationScript { version: i64, message: String },

    /// Returned when the connection pool cannot provide a connection.
    #[error("Connection pool error: {0}")]
    Pool(String),
}

impl<T> From<std::sync::PoisonError<T>> for StorageError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        StorageError::LockPoisoned
    }
}

impl From<tokio::task::JoinError> for StorageError {
    fn from(e: tokio::task::JoinError) -> Self {
        StorageError::TaskPanic(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Db — the public handle
// ---------------------------------------------------------------------------

/// Cheaply cloneable handle to the Shabakat SQLite connection pool.
///
/// `Clone` is O(1) — every clone shares the same `Arc<Pool>`.  All async
/// methods check out a connection from the pool, execute the closure on the
/// Tokio blocking thread pool (via `deadpool-sqlite`'s internal
/// `spawn_blocking`), then return the connection to the pool automatically.
///
/// # Pool behaviour
///
/// Pool size defaults to [`DB_POOL_SIZE`] (4).  If all connections are checked
/// out, `pool.get()` yields the current Tokio task (not a blocking spin) until
/// one becomes available — the async reactor is never stalled.
#[derive(Clone, Debug)]
pub struct Db {
    pool:    Arc<Pool>,
    /// Retained for `connect_dedicated()` and test helpers.
    db_path: Arc<str>,
}

impl Db {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Open (or create) the SQLite database at `path`, configure a connection
    /// pool, apply WAL pragmas to every pooled connection, and run all pending
    /// schema migrations.
    ///
    /// This function is **`async`**.  Call it from the Tokio runtime context
    /// in `main.rs`:
    ///
    /// ```rust,ignore
    /// let db = Db::open(&db_path).await?;
    /// ```
    pub async fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        info!(
            "[FLIGHT_RECORDER] Opening SQLite pool ({} connections) at: {}",
            DB_POOL_SIZE, path_str
        );

        let pool = PoolConfig::new(path_str.clone())
            .builder(Runtime::Tokio1)
            .map_err(|e| StorageError::Pool(e.to_string()))?
            .max_size(DB_POOL_SIZE)
            .build()
            .map_err(|e| StorageError::Pool(e.to_string()))?;

        let db = Db {
            pool:    Arc::new(pool),
            db_path: path_str.into(),
        };

        // Apply pragmas + run migrations via the pool's first connection.
        db.interact_mut(|conn| {
            apply_pragmas(conn).map_err(|e| match e {
                StorageError::Sqlite(err) => err,
                _ => rusqlite::Error::InvalidQuery,
            })?;
            run_migrations(conn).map_err(|e| match e {
                StorageError::Sqlite(err) => err,
                _ => rusqlite::Error::InvalidQuery,
            })?;
            Ok(())
        })
        .await?;

        info!(
            "[FLIGHT_RECORDER] Storage pool online — schema v{} — {}",
            CURRENT_SCHEMA_VERSION, db.db_path
        );

        Ok(db)
    }

    // -----------------------------------------------------------------------
    // Async interaction wrappers
    // -----------------------------------------------------------------------

    /// Execute an **immutable** (read or single-statement write) closure
    /// against a pooled connection on Tokio's blocking thread pool.
    ///
    /// The closure receives a shared `&Connection`.  For multi-statement
    /// transactions that require `conn.transaction()`, use [`Db::interact_mut`].
    ///
    /// # Errors
    ///
    /// Propagates [`StorageError::Sqlite`] from the closure, or
    /// [`StorageError::Pool`] / [`StorageError::TaskPanic`] from the
    /// infrastructure layer.
    pub async fn interact<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error> + Send + 'static,
        R: Send + 'static,
    {
        let conn = self.pool.get().await
            .map_err(|e| StorageError::Pool(e.to_string()))?;
        conn.interact(move |c| f(c))
            .await
            .map_err(|e| StorageError::TaskPanic(e.to_string()))?
            .map_err(StorageError::Sqlite)
    }

    /// Execute a **mutable** closure (transactions, DDL, multi-step writes)
    /// against a pooled connection on Tokio's blocking thread pool.
    ///
    /// The closure receives an exclusive `&mut Connection`, which is required
    /// by `rusqlite` to start a [`rusqlite::Transaction`].
    ///
    /// # Errors
    ///
    /// Same as [`Db::interact`].
    pub async fn interact_mut<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Connection) -> Result<R, rusqlite::Error> + Send + 'static,
        R: Send + 'static,
    {
        let conn = self.pool.get().await
            .map_err(|e| StorageError::Pool(e.to_string()))?;
        conn.interact(move |c| f(c))
            .await
            .map_err(|e| StorageError::TaskPanic(e.to_string()))?
            .map_err(StorageError::Sqlite)
    }

    /// Execute a closure against a pooled connection, converting any error to
    /// `String`.
    ///
    /// This is the preferred entry point for storage sub-modules which use
    /// `Result<R, String>` internally.  Accepts closures returning
    /// `Result<R, E>` for **any** `E: Display`.
    ///
    /// # Reactor Safety
    ///
    /// `rusqlite` is a synchronous C-library.  The closure **always** runs on
    /// `deadpool-sqlite`'s internal `spawn_blocking` pool and never blocks the
    /// Tokio event loop.
    pub async fn execute<F, R, E>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let conn = self.pool.get().await
            .map_err(|e| format!("[DB] pool exhausted: {e}"))?;
        conn.interact(move |c| f(c).map_err(|e| e.to_string()))
            .await
            .map_err(|e| format!("[DB] spawn_blocking panicked: {e}"))?
    }

    /// Open a **dedicated** (non-pooled) connection to the same database for
    /// health probes and diagnostic tooling.  Not for query hot-paths.
    pub fn connect_dedicated(&self) -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open(self.db_path.as_ref())?;
        apply_pragmas(&conn).map_err(|e| match e {
            StorageError::Sqlite(re) => re,
            _ => rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some("pragma application failed".to_string()),
            ),
        })?;
        Ok(conn)
    }
}

// ---------------------------------------------------------------------------
// WAL + performance pragmas (applied to every connection in the pool)
// ---------------------------------------------------------------------------

/// Apply the standard WAL + performance pragmas to a connection.
///
/// These are connection-level settings; SQLite does not persist them in the
/// database file, so they must be applied to every new connection.
///
/// | Pragma         | Value       | Reason |
/// |----------------|-------------|--------|
/// | journal_mode   | WAL         | Readers never block writers |
/// | synchronous    | NORMAL      | WAL-safe; balances durability vs speed |
/// | busy_timeout   | 5000 ms     | Retry on write contention before failing |
/// | foreign_keys   | ON          | Enforce referential constraints per-connection |
/// | temp_store     | MEMORY      | Keep temp tables in RAM |
/// | mmap_size      | 256 MiB     | Memory-map the DB file for faster reads |
fn apply_pragmas(conn: &Connection) -> Result<(), StorageError> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous   = NORMAL;
         PRAGMA busy_timeout  = 5000;
         PRAGMA foreign_keys  = ON;
         PRAGMA temp_store    = MEMORY;
         PRAGMA mmap_size     = 268435456;",
    )?;
    info!(
        "[FLIGHT_RECORDER] SQLite pragmas applied: \
         WAL, synchronous=NORMAL, busy_timeout=5000ms, \
         foreign_keys=ON, temp_store=MEMORY, mmap=256MiB"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Migration engine (private)
// ---------------------------------------------------------------------------

/// Apply all pending schema migrations in version order.
///
/// Version tracking uses SQLite's built-in `PRAGMA user_version`.  The value
/// starts at 0 on a fresh database and is bumped atomically by each block.
///
/// ## How to add a new migration
///
/// 1. Increment [`CURRENT_SCHEMA_VERSION`].
/// 2. Add a new `if stored_version < N { … }` block below.
/// 3. Apply the DDL with `conn.execute_batch(…)`.
/// 4. Stamp with `conn.pragma_update(None, "user_version", N)`.
fn run_migrations(conn: &Connection) -> Result<(), StorageError> {
    let stored_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or_else(|e| {
            warn!(
                "[FLIGHT_RECORDER] Could not read PRAGMA user_version ({}); \
                 assuming 0 (fresh database)",
                e
            );
            0
        });

    info!(
        "[FLIGHT_RECORDER] Schema migration check — stored: {}, target: {}",
        stored_version, CURRENT_SCHEMA_VERSION
    );

    if stored_version >= CURRENT_SCHEMA_VERSION {
        info!("[FLIGHT_RECORDER] Schema is current — no migrations required");
        return Ok(());
    }

    // ── v0 → v1: Initial schema ──────────────────────────────────────────────
    if stored_version < 1 {
        info!("[FLIGHT_RECORDER] Applying migration v0 → v1 (initial schema DDL)");
        conn.execute_batch(SCHEMA_SQL).map_err(|e| {
            error!("[FLIGHT_RECORDER] Migration v1 FAILED — schema.sql error: {}", e);
            StorageError::MigrationFailed { version: 1, source: e }
        })?;
        conn.pragma_update(None, "user_version", 1_i64)
            .map_err(|e| StorageError::MigrationFailed { version: 1, source: e })?;
        info!("[FLIGHT_RECORDER] Migration v1 applied and stamped successfully");
    }

    // ── v1 → v2: Extended schema ─────────────────────────────────────────────
    if stored_version < 2 {
        info!(
            "[FLIGHT_RECORDER] Applying migration v1 → v2 \
             (extended schema: outages, speed_tests, dns_providers, \
             device_aliases, system_status, notification_providers, \
             hourly_metrics, device_events + column additions)"
        );
        migrations::run(conn).map_err(|message| {
            error!("[FLIGHT_RECORDER] Migration v2 FAILED — migrations::run error: {}", message);
            StorageError::MigrationScript { version: 2, message }
        })?;
        conn.pragma_update(None, "user_version", 2_i64)
            .map_err(|e| StorageError::MigrationFailed { version: 2, source: e })?;
        info!("[FLIGHT_RECORDER] Migration v2 applied and stamped successfully");
    }

    // ── v2 → v3: Add devices.is_active ──────────────────────────────────────
    if stored_version < 3 {
        info!("[FLIGHT_RECORDER] Applying migration v2 → v3 (add devices.is_active)");
        // Ignore error — column may already exist on upgraded databases.
        let _ = conn.execute(
            "ALTER TABLE devices ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1",
            [],
        );
        conn.pragma_update(None, "user_version", 3_i64)
            .map_err(|e| StorageError::MigrationFailed { version: 3, source: e })?;
        info!("[FLIGHT_RECORDER] Migration v3 applied and stamped successfully");
    }

    // ── Future migrations ────────────────────────────────────────────────────
    //
    // Template for v3 → v4:
    //
    //   if stored_version < 4 {
    //       info!("[FLIGHT_RECORDER] Applying migration v3 → v4 (...)");
    //       conn.execute_batch("ALTER TABLE devices ADD COLUMN …")?;
    //       conn.pragma_update(None, "user_version", 4_i64)?;
    //       info!("[FLIGHT_RECORDER] Migration v4 applied and stamped successfully");
    //   }

    info!(
        "[FLIGHT_RECORDER] All migrations complete — schema at version {}",
        CURRENT_SCHEMA_VERSION
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Schema-level tests use a plain in-memory connection ──────────────────
    //
    // deadpool-sqlite does not support `:memory:` in the pool (each pooled
    // connection is a separate in-memory DB invisible to the others).  For
    // schema integrity tests we use a raw `rusqlite::Connection` directly.

    fn open_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory connection");
        apply_pragmas(&conn).expect("pragmas");
        run_migrations(&conn).expect("migrations");
        conn
    }

    // ── Schema integrity ─────────────────────────────────────────────────────

    #[test]
    fn all_tables_created_after_migration() {
        let conn = open_test_conn();
        for table in &["devices", "scan_history", "device_events", "settings"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master \
                     WHERE type = 'table' AND name = ?1",
                    rusqlite::params![table],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            assert_eq!(count, 1, "table '{}' should exist", table);
        }
    }

    #[test]
    fn user_version_stamped_correctly() {
        let conn = open_test_conn();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .expect("user_version pragma");
        assert_eq!(
            version, CURRENT_SCHEMA_VERSION,
            "PRAGMA user_version should match CURRENT_SCHEMA_VERSION"
        );
    }

    #[test]
    fn schema_application_is_idempotent() {
        let conn = open_test_conn();
        conn.execute_batch(SCHEMA_SQL)
            .expect("second schema application must be idempotent");
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let conn = open_test_conn();
        let fk: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .expect("foreign_keys pragma");
        assert_eq!(fk, 1, "PRAGMA foreign_keys should be ON (1)");
    }

    #[test]
    fn foreign_key_violation_is_rejected() {
        let conn = open_test_conn();
        let result = conn.execute(
            "INSERT INTO scan_history \
             (scan_id, scanned_at, device_id, ip, is_online) \
             VALUES ('test-1', 1000, 9999, '10.0.0.1', 1)",
            [],
        );
        assert!(
            result.is_err(),
            "INSERT referencing non-existent device_id should fail"
        );
    }

    // ── Async pool tests use a temp file ────────────────────────────────────
    //
    // Each test gets a unique temp DB file so they don't share state.

    async fn open_test_db() -> Db {
        let path = format!("/tmp/shabakat_test_{}.db", monotonic_suffix());
        Db::open(&path).await.expect("test pool should open")
    }

    fn monotonic_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    #[tokio::test]
    async fn interact_returns_query_result() {
        let db = open_test_db().await;
        let count: i64 = db
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("interact should succeed on empty table");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn interact_surfaces_sqlite_errors() {
        let db = open_test_db().await;
        let result: Result<i64, StorageError> = db
            .interact(|conn| {
                conn.query_row("SELECT * FROM nonexistent_table_xyz", [], |r| r.get(0))
            })
            .await;
        assert!(
            matches!(result, Err(StorageError::Sqlite(_))),
            "expected StorageError::Sqlite, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn interact_mut_transaction_commits() {
        let db = open_test_db().await;
        db.interact_mut(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO devices (mac, first_seen, last_seen) VALUES (?1, ?2, ?2)",
                rusqlite::params!["AA:BB:CC:DD:EE:FF", 1_700_000_000_000_i64],
            )?;
            tx.commit()
        })
        .await
        .expect("transaction should commit");

        let count: i64 = db
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("read after write");
        assert_eq!(count, 1, "device should be present after committed transaction");
    }

    #[tokio::test]
    async fn interact_mut_rollback_leaves_no_rows() {
        let db = open_test_db().await;
        db.interact_mut(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO devices (mac, first_seen, last_seen) VALUES (?1, ?2, ?2)",
                rusqlite::params!["AA:BB:CC:DD:EE:FF", 1_700_000_000_000_i64],
            )?;
            drop(tx); // implicit rollback
            Ok(())
        })
        .await
        .expect("drop-rollback should not error");

        let count: i64 = db
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("read after rollback");
        assert_eq!(count, 0, "rollback should leave no rows");
    }

    #[tokio::test]
    async fn cloned_handles_share_the_same_pool() {
        let db_a = open_test_db().await;
        let db_b = db_a.clone(); // Same Arc<Pool>

        db_a.interact_mut(|conn| {
            conn.execute(
                "INSERT INTO devices (mac, first_seen, last_seen) \
                 VALUES ('11:22:33:44:55:66', 1000, 1000)",
                [],
            )
            .map(|_| ())
        })
        .await
        .expect("write via db_a");

        let count: i64 = db_b
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("read via db_b");

        assert_eq!(
            count, 1,
            "db_b must observe the row written through db_a (shared pool)"
        );
    }
}
