use crate::storage::AppDb;
use crate::types::DnsProvider;
use libsql::{params, Row};

pub async fn add_provider(db: AppDb, provider: DnsProvider) -> Result<(), String> {
    let conn = db.connect().await?;
    conn.execute(
        "INSERT INTO dns_providers (id, name, ip, port, username, password, is_enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            provider.id,
            provider.name,
            provider.ip,
            provider.port as i64,
            provider.username,
            provider.password,
            provider.is_enabled as i64,
            provider.created_at,
        ],
    )
    .await
    .map_err(|e| format!("add provider: {e}"))?;
    Ok(())
}

pub async fn get_active_providers(db: AppDb) -> Result<Vec<DnsProvider>, String> {
    let conn = db.connect().await?;
    let mut rows = match conn
        .query("SELECT id, name, ip, port, username, password, is_enabled, created_at FROM dns_providers WHERE is_enabled = 1", ())
        .await {
            Ok(r) => r,
            Err(_) => return Ok(Vec::new()),
        };

    let mut results = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        results.push(row_to_provider(&row)?);
    }
    Ok(results)
}

pub async fn list_providers(db: AppDb) -> Result<Vec<DnsProvider>, String> {
    let conn = db.connect().await?;
    let mut rows = match conn
        .query("SELECT id, name, ip, port, username, password, is_enabled, created_at FROM dns_providers ORDER BY created_at DESC", ())
        .await {
            Ok(r) => r,
            Err(_) => return Ok(Vec::new()),
        };

    let mut results = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        results.push(row_to_provider(&row)?);
    }
    Ok(results)
}

pub async fn toggle_provider_status(db: AppDb, id: String, is_enabled: bool) -> Result<(), String> {
    let conn = db.connect().await?;
    conn.execute(
        "UPDATE dns_providers SET is_enabled = ?1 WHERE id = ?2",
        params![is_enabled as i64, id],
    )
    .await
    .map_err(|e| format!("toggle provider: {e}"))?;
    Ok(())
}

pub async fn delete_provider(db: AppDb, id: String) -> Result<(), String> {
    let conn = db.connect().await?;
    conn.execute("DELETE FROM dns_providers WHERE id = ?1", params![id])
        .await
        .map_err(|e| format!("delete provider: {e}"))?;
    Ok(())
}

fn row_to_provider(row: &Row) -> Result<DnsProvider, String> {
    Ok(DnsProvider {
        id: row.get::<Option<String>>(0).map_err(|e| e.to_string())?.unwrap_or_default(),
        name: row.get::<Option<String>>(1).map_err(|e| e.to_string())?.unwrap_or_default(),
        ip: row.get::<Option<String>>(2).map_err(|e| e.to_string())?.unwrap_or_default(),
        port: row.get::<Option<i64>>(3).map_err(|e| e.to_string())?.unwrap_or_default() as u16,
        username: row.get::<Option<String>>(4).map_err(|e| e.to_string())?,
        password: row.get::<Option<String>>(5).map_err(|e| e.to_string())?,
        is_enabled: row.get::<Option<i64>>(6).map_err(|e| e.to_string())?.unwrap_or_default() != 0,
        created_at: row.get::<Option<i64>>(7).map_err(|e| e.to_string())?.unwrap_or_default(),
    })
}
