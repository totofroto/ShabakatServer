//! # `storage` — Thread-Safe SQLite Persistence Layer
//!
//! ## Design Contract
//!
//! This module owns the **only** live database connection for the entire
//! Shabakat Server process.  It exposes a single [`Db`] handle that is:
//!
//! - **Cheaply cloneable** — backed by `Arc<Mutex<rusqlite::Connection>>`,
//!   so every Axum route handler and background Tokio task can hold a copy
//!   without any additional setup.
//!
//! - **Async-reactor safe** — `rusqlite` is a *synchronous, blocking*
//!   C-library driver.  Every query is dispatched through
//!   [`tokio::task::spawn_blocking`] so the Tokio event loop (and the four
//!   cores of the Celeron J4125) are never starved while SQLite holds an
//!   OS page-cache lock.
//!
//! - **Self-migrating** — [`Db::open`] reads the embedded `schema.sql`
//!   at startup and applies any pending migrations tracked by SQLite's
//!   built-in `PRAGMA user_version`.  All `CREATE TABLE` statements in the
//!   schema use `IF NOT EXISTS`, making the process fully idempotent.
//!
//! ## Usage Pattern
//!
//! ```rust,ignore
//! // Startup (synchronous, before Axum router is bound):
//! let db = Db::open("/data/shabakat.db")?;
//!
//! // In an Axum handler or background task:
//! let count: i64 = db
//!     .interact(|conn| {
//!         conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0))
//!     })
//!     .await?;
//!
//! // Transaction (requires mutable connection):
//! db.interact_mut(|conn| {
//!     let tx = conn.transaction()?;
//!     tx.execute(
//!         "INSERT OR IGNORE INTO devices (mac, first_seen, last_seen) VALUES (?1, ?2, ?2)",
//!         rusqlite::params!["AA:BB:CC:DD:EE:FF", 1_700_000_000_000_i64],
//!     )?;
//!     tx.commit()
//! })
//! .await?;
//! ```

// ---------------------------------------------------------------------------
// Sub-modules — each gets its own source file under src/storage/
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
use std::sync::{Arc, Mutex, MutexGuard};

use log::{error, info, warn};
use rusqlite::Connection;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Embedded schema — compiled directly into the binary so the container image
// needs no external SQL files.  Path is relative to this source file.
// ---------------------------------------------------------------------------
const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Logical schema version.  **Increment this whenever a new migration block
/// is added to [`run_migrations`].**  The value is persisted via
/// `PRAGMA user_version` so it survives container restarts.
///
/// v1 — base 4-table schema (devices, scan_history, device_events, settings)
/// v2 — extended schema: outages, speed_tests, dns_providers, device_aliases,
///       system_status, notification_providers, hourly_metrics + ALTER TABLE
///       column additions via `migrations::run`.
const CURRENT_SCHEMA_VERSION: i64 = 2;

// ---------------------------------------------------------------------------
// Public type alias + utility function used by every sub-module
// ---------------------------------------------------------------------------

/// Type alias so sub-modules (and the rest of the crate) can write `AppDb`
/// instead of `Db`.  They are the same type — `AppDb` is cheaper to type and
/// conveys "application-level handle" vs the lower-level struct name.
pub type AppDb = Db;

/// Returns the current wall-clock time as milliseconds since the Unix epoch.
///
/// Used throughout the storage layer to timestamp events without pulling in
/// the `chrono` crate at every call site.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// All errors that can originate from the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// A `rusqlite` / SQLite engine error.
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The `Mutex` protecting the connection was poisoned — another thread
    /// panicked while holding the lock.  This is a fatal condition; restart
    /// the container.
    #[error(
        "Database connection mutex is poisoned \
         (a previous thread panicked while holding the lock)"
    )]
    LockPoisoned,

    /// A `spawn_blocking` task panicked.  The message contains the panic
    /// payload as a string.
    #[error("Blocking database task panicked: {0}")]
    TaskPanic(String),

    /// A schema migration step failed (rusqlite-level error).
    #[error("Schema migration to version {version} failed: {source}")]
    MigrationFailed {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },

    /// A schema migration step returned a plain string error (from
    /// `migrations::run` which uses `Result<(), String>`).
    #[error("Schema migration to version {version} script error: {message}")]
    MigrationScript { version: i64, message: String },
}

// Manual From impls so callers can use `?` cleanly -------------------------

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

/// Thread-safe, cheaply-cloneable handle to the Shabakat SQLite database.
///
/// Clone this freely — each clone shares the same underlying connection
/// via the inner `Arc`.  The connection is protected by a `Mutex` so only
/// one blocking closure executes at a time (SQLite in WAL mode serialises
/// writers; concurrent readers are naturally safe at the SQLite level, but
/// the single-connection model simplifies the Rust ownership story).
///
/// All mutations are issued through [`Db::interact_mut`] which holds the
/// mutex only on the blocking thread pool, never on the Tokio event loop.
#[derive(Clone, Debug)]
pub struct Db {
    inner: Arc<Mutex<Connection>>,
}

impl Db {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Open (or create) the SQLite database at `path`, apply the WAL
    /// configuration pragmas, and run all pending schema migrations.
    ///
    /// **This is a synchronous function.**  Call it once during application
    /// startup, *before* binding the Axum router.  All subsequent access
    /// goes through the async [`Db::interact`] / [`Db::interact_mut`] API.
    ///
    /// Pass `":memory:"` for an in-process, non-persistent database useful
    /// in unit tests.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the database file cannot be opened, the
    /// WAL pragmas fail, or any migration step fails.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let path = path.as_ref();
        info!(
            "[FLIGHT_RECORDER] Opening SQLite database at path: {}",
            path.display()
        );

        let conn = Connection::open(path)?;

        // ── WAL + performance pragmas ──────────────────────────────────────
        // These are connection-level settings — SQLite does not persist them
        // in the database file, so they must be applied on every new
        // connection.
        //
        //   journal_mode = WAL
        //     Write-Ahead Logging: readers never block writers and vice-versa.
        //     Dramatically reduces lock contention when background scanner
        //     tasks insert rows while the HTTP layer reads them.
        //
        //   synchronous = NORMAL
        //     Recommended companion to WAL.  Durability is guaranteed up to
        //     the last committed transaction on OS crash; only a power-loss
        //     in the sub-second window before fsync could lose data.
        //
        //   busy_timeout = 5000
        //     If another write is in-flight, retry for up to 5 seconds
        //     before returning SQLITE_BUSY.  Prevents spurious errors when
        //     the heartbeat scheduler and the HTTP API write simultaneously.
        //
        //   foreign_keys = ON
        //     Must be set per-connection in SQLite; it is NOT a stored
        //     database flag.  Enforces the referential constraints defined
        //     in schema.sql (e.g. scan_history.device_id → devices.id).
        //
        //   temp_store = MEMORY
        //     Keep temporary tables and indices in RAM instead of on disk.
        //
        //   mmap_size = 268435456 (256 MiB)
        //     Memory-map the database file; faster sequential reads on the
        //     NAS's spinning-disk or SSD-cached array.
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
             WAL mode, synchronous=NORMAL, busy_timeout=5000ms, \
             foreign_keys=ON, temp_store=MEMORY, mmap=256MiB"
        );

        // ── Schema migrations ──────────────────────────────────────────────
        run_migrations(&conn)?;

        info!(
            "[FLIGHT_RECORDER] Storage layer ready — \
             schema version {} — database: {}",
            CURRENT_SCHEMA_VERSION,
            path.display()
        );

        Ok(Db {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    // -----------------------------------------------------------------------
    // Async interaction wrappers
    // -----------------------------------------------------------------------

    /// Execute an **immutable** (read or single-statement write) closure
    /// against the database connection on Tokio's blocking thread pool.
    ///
    /// The closure receives a shared `&Connection`.  For multi-statement
    /// transactions that require `conn.transaction()`, use [`Db::interact_mut`].
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let count: i64 = db
    ///     .interact(|conn| {
    ///         conn.query_row(
    ///             "SELECT COUNT(*) FROM devices WHERE acknowledged = 0",
    ///             [],
    ///             |row| row.get(0),
    ///         )
    ///     })
    ///     .await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Propagates [`StorageError::Sqlite`] from the closure, or
    /// [`StorageError::LockPoisoned`] / [`StorageError::TaskPanic`] from
    /// the infrastructure layer.
    pub async fn interact<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error> + Send + 'static,
        R: Send + 'static,
    {
        let handle = Arc::clone(&self.inner);

        tokio::task::spawn_blocking(move || {
            // Acquire the lock synchronously on the blocking thread.
            // This is safe: we are NOT on the Tokio event loop.
            let guard: MutexGuard<Connection> =
                handle.lock().map_err(|_| StorageError::LockPoisoned)?;

            f(&guard).map_err(StorageError::Sqlite)
        })
        .await
        .map_err(StorageError::from)? // JoinError (task panicked)
        // Inner Result<R, StorageError> from the closure
    }

    /// Execute a **mutable** closure (transactions, DDL, multi-step writes)
    /// against the database connection on Tokio's blocking thread pool.
    ///
    /// The closure receives an exclusive `&mut Connection`, which is required
    /// by `rusqlite` to start a [`rusqlite::Transaction`].
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// db.interact_mut(|conn| {
    ///     let tx = conn.transaction()?;
    ///     tx.execute(
    ///         "INSERT OR REPLACE INTO devices \
    ///          (mac, first_seen, last_seen, last_ip) \
    ///          VALUES (?1, ?2, ?3, ?4)",
    ///         rusqlite::params![mac, now_ms, now_ms, ip],
    ///     )?;
    ///     tx.execute(
    ///         "INSERT INTO scan_history \
    ///          (scan_id, scanned_at, device_id, ip, is_online) \
    ///          VALUES (?1, ?2, last_insert_rowid(), ?3, 1)",
    ///         rusqlite::params![scan_id, now_ms, ip],
    ///     )?;
    ///     tx.commit()
    /// })
    /// .await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Same as [`Db::interact`].
    pub async fn interact_mut<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Connection) -> Result<R, rusqlite::Error> + Send + 'static,
        R: Send + 'static,
    {
        let handle = Arc::clone(&self.inner);

        tokio::task::spawn_blocking(move || {
            let mut guard: MutexGuard<Connection> =
                handle.lock().map_err(|_| StorageError::LockPoisoned)?;

            f(&mut guard).map_err(StorageError::Sqlite)
        })
        .await
        .map_err(StorageError::from)?
    }

    // -----------------------------------------------------------------------
    // Ergonomic wrapper used by the higher-level sub-modules
    // -----------------------------------------------------------------------

    /// Execute a closure against the database connection on Tokio's blocking
    /// thread pool, converting any error to `String`.
    ///
    /// This is the preferred entry point for the storage sub-modules
    /// (`compactor`, `devices`, `history`, etc.) which all use
    /// `Result<R, String>` internally rather than `Result<R, StorageError>`.
    ///
    /// The generic error bound `E: Display` means the same method accepts
    /// closures that return either `Result<R, String>` *or*
    /// `Result<R, rusqlite::Error>` — no `.map_err` boilerplate needed at
    /// call sites that discard the return value.
    ///
    /// # Reactor Safety
    ///
    /// `rusqlite` is a synchronous C-library.  The closure **always** runs on
    /// `tokio::task::spawn_blocking`'s dedicated thread pool and never blocks
    /// the async reactor.
    /// Execute a closure against the database on the blocking thread pool,
    /// converting any error to `String` via its `Display` impl.
    ///
    /// Accepts closures that return `Result<R, E>` for **any** `E: Display`:
    ///  - `Result<R, String>` — storage sub-modules
    ///  - `Result<R, rusqlite::Error>` — API handlers that use `?` directly
    ///
    /// For closures whose return type Rust cannot infer (e.g. those that only
    /// ever hit the `Ok(())` path), annotate the closure explicitly:
    /// ```rust,ignore
    /// db.execute(|conn| -> Result<(), String> { … Ok(()) })
    /// ```
    pub async fn execute<F, R, E>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let handle = Arc::clone(&self.inner);

        tokio::task::spawn_blocking(move || {
            let guard: MutexGuard<Connection> =
                handle.lock().map_err(|_| "[DB] mutex poisoned".to_string())?;
            f(&guard).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("[DB] spawn_blocking panicked: {e}"))?
    }

    /// Open a **dedicated** (non-shared) connection to the same database file
    /// for health probes and diagnostic tooling.  Not for query hot-paths.
    pub fn connect_dedicated(&self) -> Result<Connection, rusqlite::Error> {
        let path = {
            let guard = self.inner.lock().map_err(|_| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some("mutex poisoned".to_string()),
                )
            })?;
            // Execute a no-op query to retrieve the database filename.
            guard.query_row("PRAGMA database_list", [], |row| {
                row.get::<_, String>(2)
            })?
        };
        Connection::open(path)
    }
}

// ---------------------------------------------------------------------------
// Migration engine (private)
// ---------------------------------------------------------------------------

/// Apply all pending schema migrations in version order.
///
/// Version tracking uses SQLite's built-in `PRAGMA user_version`.  The value
/// starts at 0 on a fresh database and is bumped atomically by each
/// migration block.
///
/// ## How to add a new migration
///
/// 1. Increment [`CURRENT_SCHEMA_VERSION`].
/// 2. Add a new `if current_version < N { … }` block below the existing ones.
/// 3. Apply the DDL with `conn.execute_batch(…)`.
/// 4. Stamp the new version with `conn.pragma_update(None, "user_version", N)`.
///
/// Keep each block idempotent where possible (`ALTER TABLE … IF NOT EXISTS`
/// is not valid in SQLite for column additions, but you can guard with a
/// `SELECT COUNT(*) FROM pragma_table_info(…)` check if needed).
fn run_migrations(conn: &Connection) -> Result<(), StorageError> {
    // Read the version that is currently stamped in the database file.
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
        "[FLIGHT_RECORDER] Schema migration check — \
         stored version: {}, target version: {}",
        stored_version, CURRENT_SCHEMA_VERSION
    );

    if stored_version >= CURRENT_SCHEMA_VERSION {
        info!("[FLIGHT_RECORDER] Schema is current — no migrations required");
        return Ok(());
    }

    // ── v0 → v1: Initial schema (devices, scan_history, device_events, settings)
    if stored_version < 1 {
        info!("[FLIGHT_RECORDER] Applying migration v0 → v1 (initial schema DDL)");

        conn.execute_batch(SCHEMA_SQL).map_err(|e| {
            error!(
                "[FLIGHT_RECORDER] Migration v1 FAILED — schema.sql execution error: {}",
                e
            );
            StorageError::MigrationFailed {
                version: 1,
                source: e,
            }
        })?;

        // Stamp the version only after the migration succeeds, so a crash
        // mid-migration will cause a clean retry on the next startup.
        conn.pragma_update(None, "user_version", 1_i64)
            .map_err(|e| {
                error!(
                    "[FLIGHT_RECORDER] Failed to stamp schema version 1: {}",
                    e
                );
                StorageError::MigrationFailed {
                    version: 1,
                    source: e,
                }
            })?;

        info!("[FLIGHT_RECORDER] Migration v1 applied and stamped successfully");
    }

    // ── v1 → v2: Extended schema (additional tables + ALTER TABLE columns) ───
    //
    // `migrations::run` is idempotent: every CREATE uses IF NOT EXISTS and
    // every ALTER TABLE failure is silently ignored (SQLite rejects duplicate
    // column additions).  Running it on a v1 database is always safe.
    if stored_version < 2 {
        info!(
            "[FLIGHT_RECORDER] Applying migration v1 → v2 \
             (extended schema: outages, speed_tests, dns_providers, \
             device_aliases, system_status, notification_providers, \
             hourly_metrics, device_events + column additions)"
        );

        migrations::run(conn).map_err(|message| {
            error!(
                "[FLIGHT_RECORDER] Migration v2 FAILED — migrations::run error: {}",
                message
            );
            StorageError::MigrationScript { version: 2, message }
        })?;

        conn.pragma_update(None, "user_version", 2_i64)
            .map_err(|e| {
                error!(
                    "[FLIGHT_RECORDER] Failed to stamp schema version 2: {}",
                    e
                );
                StorageError::MigrationFailed { version: 2, source: e }
            })?;

        info!("[FLIGHT_RECORDER] Migration v2 applied and stamped successfully");
    }

    // ── Future migrations ────────────────────────────────────────────────────
    //
    // Example v3 → v4:
    //
    //   if stored_version < 4 {
    //       info!("[FLIGHT_RECORDER] Applying migration v3 → v4 (add devices.is_gateway)");
    //       conn.execute_batch(
    //           "ALTER TABLE devices ADD COLUMN is_gateway INTEGER NOT NULL DEFAULT 0;"
    //       )
    //       .map_err(|e| StorageError::MigrationFailed { version: 4, source: e })?;
    //       conn.pragma_update(None, "user_version", 4_i64)
    //           .map_err(|e| StorageError::MigrationFailed { version: 4, source: e })?;
    //       info!("[FLIGHT_RECORDER] Migration v4 applied and stamped successfully");
    //   }

    info!(
        "[FLIGHT_RECORDER] All migrations complete — \
         schema is now at version {}",
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

    /// Helper: open a fresh in-memory database and run migrations.
    fn open_test_db() -> Db {
        Db::open(":memory:").expect("in-memory database should open cleanly")
    }

    // ── Schema integrity ─────────────────────────────────────────────────────

    /// All four tables must exist after a fresh migration.
    #[test]
    fn all_tables_created_after_migration() {
        let db = open_test_db();
        let conn = db.inner.lock().expect("lock");

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

    /// `PRAGMA user_version` must equal [`CURRENT_SCHEMA_VERSION`] after
    /// a successful migration.
    #[test]
    fn user_version_stamped_correctly() {
        let db = open_test_db();
        let conn = db.inner.lock().expect("lock");

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .expect("user_version pragma");

        assert_eq!(
            version, CURRENT_SCHEMA_VERSION,
            "PRAGMA user_version should match CURRENT_SCHEMA_VERSION after migration"
        );
    }

    /// Re-applying the schema SQL a second time must not return an error
    /// (all statements are `CREATE … IF NOT EXISTS`).
    #[test]
    fn schema_application_is_idempotent() {
        let db = open_test_db();
        let conn = db.inner.lock().expect("lock");

        // Applying the embedded schema again must not fail.
        conn.execute_batch(SCHEMA_SQL)
            .expect("second schema application must be idempotent");
    }

    /// `foreign_keys` pragma must be ON after opening the database.
    #[test]
    fn foreign_keys_are_enforced() {
        let db = open_test_db();
        let conn = db.inner.lock().expect("lock");

        let fk_status: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .expect("foreign_keys pragma");

        assert_eq!(fk_status, 1, "PRAGMA foreign_keys should be ON (1)");
    }

    /// Inserting a `scan_history` row that references a non-existent
    /// `device_id` must be rejected by the foreign-key constraint.
    #[test]
    fn foreign_key_violation_is_rejected() {
        let db = open_test_db();
        let conn = db.inner.lock().expect("lock");

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

    // ── Async interact wrappers ──────────────────────────────────────────────

    /// A successful `interact` call must return the query result.
    #[tokio::test]
    async fn interact_returns_query_result() {
        let db = open_test_db();

        let count: i64 = db
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("interact should succeed on empty table");

        assert_eq!(count, 0);
    }

    /// A failing query inside `interact` must surface as `StorageError::Sqlite`.
    #[tokio::test]
    async fn interact_surfaces_sqlite_errors() {
        let db = open_test_db();

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

    /// `interact_mut` must be able to open a transaction, write a row, and
    /// commit it — all via the blocking wrapper.
    #[tokio::test]
    async fn interact_mut_transaction_commits() {
        let db = open_test_db();

        // Insert a device via a transaction.
        db.interact_mut(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO devices (mac, first_seen, last_seen) \
                 VALUES (?1, ?2, ?2)",
                rusqlite::params!["AA:BB:CC:DD:EE:FF", 1_700_000_000_000_i64],
            )?;
            tx.commit()
        })
        .await
        .expect("transaction should commit");

        // Verify the row exists via a read-only interact.
        let count: i64 = db
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("read after write");

        assert_eq!(count, 1, "device should be present after committed transaction");
    }

    /// A rolled-back transaction must leave the table empty.
    #[tokio::test]
    async fn interact_mut_rollback_leaves_no_rows() {
        let db = open_test_db();

        let result = db
            .interact_mut(|conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO devices (mac, first_seen, last_seen) \
                     VALUES (?1, ?2, ?2)",
                    rusqlite::params!["AA:BB:CC:DD:EE:FF", 1_700_000_000_000_i64],
                )?;
                // Intentionally do not commit — drop rolls back automatically.
                drop(tx);
                Ok(())
            })
            .await;

        // The drop-rollback itself is not an error.
        assert!(result.is_ok());

        let count: i64 = db
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("read after rollback");

        assert_eq!(count, 0, "rollback should leave no rows");
    }

    /// Cloning the handle and using both clones concurrently must not cause
    /// data races or panics (the Mutex serialises access).
    #[tokio::test]
    async fn cloned_handles_share_the_same_connection() {
        let db_a = open_test_db();
        let db_b = db_a.clone(); // Same Arc<Mutex<Connection>>

        // Write via db_a.
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

        // Read via db_b — must see the row inserted through db_a.
        let count: i64 = db_b
            .interact(|conn| conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .await
            .expect("read via db_b");

        assert_eq!(
            count, 1,
            "db_b must observe the row written through db_a (shared connection)"
        );
    }
}
