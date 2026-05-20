use log::warn;

pub async fn send_telegram(token: &str, chat_id: &str, text: &str) {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("[TELEGRAM] client build failed: {e}");
            return;
        }
    };
    if let Err(e) = client
        .post(&url)
        .json(&serde_json::json!({ "chat_id": chat_id, "text": text }))
        .send()
        .await
    {
        warn!("[TELEGRAM] send failed: {e}");
    }
}
