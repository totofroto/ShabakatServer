use crate::storage::AppDb;
use libsql::params;
use serde_json::Value;

pub async fn get_setting(db: AppDb, key: &str) -> Result<Option<String>, String> {
    let conn = db.connect().await?;
    let mut rows = match conn
        .query("SELECT value FROM settings WHERE key = ?1", params![key])
        .await {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

    match rows.next().await {
        Ok(Some(row)) => {
            let value: String = row.get::<Option<String>>(0).map_err(|e| e.to_string())?.unwrap_or_default();
            Ok(Some(value))
        }
        _ => Ok(None),
    }
}

pub async fn set_setting(db: AppDb, key: String, value: String) -> Result<(), String> {
    let conn = db.connect().await?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .await
    .map_err(|e| format!("set setting: {e}"))?;
    Ok(())
}

pub async fn get_all_settings(db: AppDb) -> Result<Value, String> {
    let conn = db.connect().await?;
    let mut rows = match conn
        .query("SELECT key, value FROM settings", ())
        .await {
            Ok(r) => r,
            Err(_) => return Ok(serde_json::Value::Object(serde_json::Map::new())),
        };

    let mut map = serde_json::Map::new();
    while let Ok(Some(row)) = rows.next().await {
        let key: String = row.get::<Option<String>>(0).map_err(|e| e.to_string())?.unwrap_or_default();
        let value: String = row.get::<Option<String>>(1).map_err(|e| e.to_string())?.unwrap_or_default();
        map.insert(key, serde_json::Value::String(value));
    }

    Ok(serde_json::Value::Object(map))
}
