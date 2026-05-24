use async_trait::async_trait;
use serde_json::Value;
use crate::notifications::NotificationProvider;
use log::info;

pub struct WebhookProvider;

#[async_trait]
impl NotificationProvider for WebhookProvider {
    async fn dispatch(&self, title: &str, body: &str, config: &Value) -> Result<(), String> {
        let url = config["url"].as_str()
            .ok_or_else(|| "Missing url for Webhook".to_string())?;
        let auth_token = config["auth_token"].as_str();

        let client = reqwest::Client::new();
        let mut request = client.post(url)
            .json(&serde_json::json!({
                "topic": "shabakat",
                "title": title,
                "message": body,
                "priority": 4,
                "tags": ["network", "alert"]
            }));

        if let Some(token) = auth_token {
            if !token.is_empty() {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
        }

        let res = request.send()
            .await
            .map_err(|e| format!("Webhook request failed: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("Webhook API error: Status {}", res.status()));
        }

        info!("[FLIGHT_RECORDER] Webhook alert transmitted successfully.");
        Ok(())
    }
}
