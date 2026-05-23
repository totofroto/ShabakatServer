use serde::{Deserialize, Serialize};
use log::{info, error};
use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPayload {
    pub title: String,
    pub mac: String,
    pub ip: String,
    pub vendor: String,
    pub hostname: Option<String>,
    pub timestamp: String,
}

pub struct NotificationDispatcher {
    client: reqwest::Client,
}

impl NotificationDispatcher {
    pub fn new() -> Self {
        Self {
            // Instantiate an optimized, connection-pooled client framework
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Dispatches structured alerts across all configured channels simultaneously
    pub async fn broadcast_alert(&self, config: &Config, payload: &AlertPayload) {
        let msg = format!(
            "🔴 <b>{}</b>\n\n\
             <b>MAC:</b> <code>{}</code>\n\
             <b>IP:</b> {}\n\
             <b>Vendor:</b> {}\n\
             <b>Hostname:</b> {}\n\
             <b>Detected At:</b> {}\n\n\
             <i>Shabakat Server Engine · Monitoring Live</i>",
            payload.title,
            payload.mac,
            payload.ip,
            payload.vendor,
            payload.hostname.as_deref().unwrap_or("Unknown"),
            payload.timestamp
        );

        self.broadcast_text(config, &msg).await;

        // Expose support for Generic Webhooks (Ntfy, Discord, Slack, etc.)
        if let Some(webhook_url) = config.webhook_url.clone() {
            let w_client = self.client.clone();
            let w_payload = payload.clone();
            
            tokio::spawn(async move {
                let res = w_client.post(&webhook_url)
                    .json(&w_payload)
                    .send()
                    .await;

                if let Err(e) = res {
                    error!("[FLIGHT_RECORDER] Generic webhook delivery channel failed: {}", e);
                }
            });
        }
    }

    /// Dispatches plain text messages across all configured channels
    pub async fn broadcast_text(&self, config: &Config, msg: &str) {
        // Task-isolate dispatch channels so one slow endpoint can never drag down the system loop
        let t_client = self.client.clone();
        let t_token = config.telegram_bot_token.clone();
        let t_chat = config.telegram_chat_id.clone();
        let t_msg = msg.to_string();

        tokio::spawn(async move {
            if let (Some(token), Some(chat_id)) = (t_token, t_chat) {
                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                let res = t_client.post(&url)
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "text": t_msg,
                        "parse_mode": "HTML"
                    }))
                    .send()
                    .await;

                match res {
                    Ok(resp) if resp.status().is_success() => {
                        info!("[FLIGHT_RECORDER] Telegram alert transmitted.");
                    }
                    Ok(resp) => {
                        error!("[FLIGHT_RECORDER] Telegram server rejected message payload: Status {}", resp.status());
                    }
                    Err(e) => {
                        error!("[FLIGHT_RECORDER] Failed hitting Telegram gateway endpoint: {}", e);
                    }
                }
            }
        });
    }
}
