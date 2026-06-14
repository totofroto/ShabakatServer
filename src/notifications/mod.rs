use async_trait::async_trait;
use serde_json::Value;
use crate::storage::AppDb;

pub mod telegram;
pub mod smtp;
pub mod webhook;

#[async_trait]
pub trait NotificationProvider: Send + Sync {
    /// Dispatches a formatted structural text string over the specific channel
    async fn dispatch(&self, title: &str, body: &str, config: &Value) -> Result<(), String>;
}

#[derive(Clone)]
pub struct NotificationDispatcher;

impl NotificationDispatcher {
    pub fn new() -> Self {
        Self
    }

    /// Compatibility wrapper for the new trait-based system
    pub async fn broadcast_alert(&self, db: &AppDb, title: &str, body: &str) {
        broadcast_alert(db, title, body).await;
    }

    pub async fn broadcast_text(&self, db: &AppDb, body: &str) {
        broadcast_alert(db, "System Alert", body).await;
    }
}

/// Dynamic alert broker that reads live database states and broadcasts alerts to all active vectors
pub async fn broadcast_alert(db: &AppDb, title: &str, body: &str) {
    // Fetch all enabled providers from the SQLite registry
    let res: Result<Vec<(String, Value)>, String> = db.execute(move |conn| -> Result<Vec<(String, Value)>, String> {
        let mut stmt = conn.prepare("SELECT id, config_json FROM notification_providers WHERE enabled = 1").map_err(|e| e.to_string())?;

        let rows = stmt.query_map([], |row| {
            let provider_id: String = row.get(0).unwrap_or_default();
            let config_str: String = row.get(1).unwrap_or_default();
            let config_json: Value = serde_json::from_str(&config_str).unwrap_or(Value::Null);
            Ok((provider_id, config_json))
        }).map_err(|e| e.to_string())?;

        let mut providers = Vec::new();
        for p in rows.flatten() {
            providers.push(p);
        }
        Ok(providers)
    }).await;

    let active_providers = match res {
        Ok(p) => p,
        Err(e) => {
            log::error!("[FLIGHT_RECORDER] Failed to read active alert providers: {}", e);
            return;
        }
    };

    for (provider_id, config_json) in active_providers {
        let provider: Option<Box<dyn NotificationProvider>> = match provider_id.as_str() {
            "telegram" => Some(Box::new(telegram::TelegramProvider)),
            "smtp" => Some(Box::new(smtp::SmtpProvider)),
            "webhook_ntfy" => Some(Box::new(webhook::WebhookProvider)),
            _ => None,
        };

        if let Some(p) = provider {
            let t = title.to_string();
            let b = body.to_string();
            let p_id = provider_id.clone();
            
            // Decouple actual network I/O from the caller thread
            tokio::spawn(async move {
                if let Err(err) = p.dispatch(&t, &b, &config_json).await {
                    log::error!("[FLIGHT_RECORDER] Provider '{}' failed to deliver alert: {}", p_id, err);
                }
            });
        }
    }
}

