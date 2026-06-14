use crate::storage::AppDb;
use rusqlite::params;
use serde_json::Value;

pub async fn set_setting(db: AppDb, key: String, value: String) -> Result<(), String> {
    db.execute(move |conn| -> Result<(), String> {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .map_err(|e| format!("set setting: {e}"))?;
        Ok(())
    }).await
}

pub async fn get_all_settings(db: AppDb) -> Result<Value, String> {
    db.execute(move |conn| -> Result<Value, String> {
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|e| e.to_string())?;

        let rows = stmt.query_map([], |row| {
            let key: String = row.get::<_, String>(0).unwrap_or_default();
            let value: String = row.get::<_, String>(1).unwrap_or_default();
            Ok((key, value))
        }).map_err(|e| e.to_string())?;

        let mut map = serde_json::Map::new();
        for (key, value) in rows.flatten() {
            map.insert(key, serde_json::Value::String(value));
        }

        Ok(serde_json::Value::Object(map))
    }).await
}

#[allow(dead_code)]
pub async fn get_setting(db: AppDb, key: &str) -> Result<Option<String>, String> {
    let key_owned = key.to_string();
    db.execute(move |conn| -> Result<Option<String>, String> {
        let value: Option<String> = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key_owned],
            |row| row.get(0)
        ).ok();
        Ok(value)
    }).await
}

