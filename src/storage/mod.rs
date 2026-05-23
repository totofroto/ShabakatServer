pub mod history;
pub mod devices;
pub mod metrics;
pub mod migrations;
pub mod networks;
pub mod providers;
pub mod settings;
pub mod system_status;

use libsql::{Builder, Connection, Database};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Clone)]
pub struct AppDb {
    pub db: Arc<Database>,
    pub conn: Arc<Mutex<Connection>>,
}

impl AppDb {
    pub async fn new(db_path: &str) -> Self {
        let db = Builder::new_local(db_path)
            .build()
            .await
            .expect("failed to open database");

        let conn = db.connect().expect("failed to connect to database");

        // Step 1: Enable WAL mode as requested by user
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .await
            .expect("failed to set WAL mode");

        conn.execute_batch(
            "PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )
        .await
        .expect("failed to set pragmas");

        migrations::run(&conn).await.expect("migrations failed");

        AppDb {
            db: Arc::new(db),
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    pub async fn connect(&self) -> Result<MutexGuard<'_, Connection>, String> {
        Ok(self.conn.lock().await)
    }

    pub fn connect_dedicated(&self) -> Result<Connection, String> {
        self.db.connect().map_err(|e| e.to_string())
    }

    pub async fn insert_log(&self, event_type: &str, details: &str) -> Result<(), String> {
        let conn = self.connect().await?;
        let now = now_ms();

        conn.execute(
            "INSERT INTO device_events (event_type, details, timestamp) VALUES (?1, ?2, ?3)",
            libsql::params![event_type, details, now],
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
