use async_trait::async_trait;
use serde_json::Value;
use crate::notifications::NotificationProvider;
use log::info;

pub struct TelegramProvider;

#[async_trait]
impl NotificationProvider for TelegramProvider {
    async fn dispatch(&self, title: &str, body: &str, config: &Value) -> Result<(), String> {
        let bot_token = config["bot_token"].as_str()
            .ok_or_else(|| "Missing bot_token for Telegram".to_string())?;
        let chat_id = config["chat_id"].as_str()
            .ok_or_else(|| "Missing chat_id for Telegram".to_string())?;

        let client = reqwest::Client::new();
        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
        
        let message = format!("<b>{}</b>\n\n{}", title, body);

        let res = client.post(&url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": message,
                "parse_mode": "HTML"
            }))
            .send()
            .await
            .map_err(|e| format!("Telegram request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("Telegram API error ({}): {}", status, err_body));
        }

        info!("[FLIGHT_RECORDER] Telegram alert transmitted successfully.");
        Ok(())
    }
}
