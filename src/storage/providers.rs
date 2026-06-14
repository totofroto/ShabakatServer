use crate::storage::AppDb;
use crate::types::DnsProvider;
use rusqlite::{params, Row};

pub async fn add_provider(db: AppDb, provider: DnsProvider) -> Result<(), String> {
    db.execute(move |conn| -> Result<(), String> {
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
        .map_err(|e| format!("add provider: {e}"))?;
        Ok(())
    }).await
}

pub async fn get_active_providers(db: AppDb) -> Result<Vec<DnsProvider>, String> {
    db.execute(move |conn| -> Result<Vec<DnsProvider>, String> {
        let mut stmt = conn
            .prepare("SELECT id, name, ip, port, username, password, is_enabled, created_at FROM dns_providers WHERE is_enabled = 1")
            .map_err(|e| e.to_string())?;
        
        let rows = stmt.query_map([], |row| {
            row_to_provider(row)
        }).map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for p in rows.flatten() {
            results.push(p);
        }
        Ok(results)
    }).await
}

pub async fn list_providers(db: AppDb) -> Result<Vec<DnsProvider>, String> {
    db.execute(move |conn| -> Result<Vec<DnsProvider>, String> {
        let mut stmt = conn
            .prepare("SELECT id, name, ip, port, username, password, is_enabled, created_at FROM dns_providers ORDER BY created_at DESC")
            .map_err(|e| e.to_string())?;
        
        let rows = stmt.query_map([], |row| {
            row_to_provider(row)
        }).map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for p in rows.flatten() {
            results.push(p);
        }
        Ok(results)
    }).await
}

pub async fn toggle_provider_status(db: AppDb, id: String, is_enabled: bool) -> Result<(), String> {
    db.execute(move |conn| -> Result<(), String> {
        conn.execute(
            "UPDATE dns_providers SET is_enabled = ?1 WHERE id = ?2",
            params![is_enabled as i64, id],
        )
        .map_err(|e| format!("toggle provider: {e}"))?;
        Ok(())
    }).await
}

pub async fn delete_provider(db: AppDb, id: String) -> Result<(), String> {
    db.execute(move |conn| -> Result<(), String> {
        conn.execute("DELETE FROM dns_providers WHERE id = ?1", params![id])
            .map_err(|e| format!("delete provider: {e}"))?;
        Ok(())
    }).await
}

fn row_to_provider(row: &Row) -> Result<DnsProvider, rusqlite::Error> {
    Ok(DnsProvider {
        id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
        name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ip: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        port: row.get::<_, Option<i64>>(3)?.unwrap_or_default() as u16,
        username: row.get::<_, Option<String>>(4)?,
        password: row.get::<_, Option<String>>(5)?,
        is_enabled: row.get::<_, Option<i64>>(6)?.unwrap_or_default() != 0,
        created_at: row.get::<_, Option<i64>>(7)?.unwrap_or_default(),
    })
}

