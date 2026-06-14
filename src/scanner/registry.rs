use std::time::Duration;
use tokio::time::interval;
use log::{info, error};
use std::fs;
use std::path::Path;

pub fn start_sync_task() {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(3600)); // Every hour
        
        loop {
            interval.tick().await;
            info!("[SYNC] Starting GitHub fingerprints sync...");
            
            match sync_fingerprints().await {
                Ok(_) => info!("[SYNC] Fingerprints synchronized successfully."),
                Err(e) => error!("[SYNC] Failed to sync fingerprints: {}", e),
            }
        }
    });
}

async fn sync_fingerprints() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "https://raw.githubusercontent.com/totofroto/ShabakatServer/main/resources/fingerprints.json";
    let response = reqwest::get(url).await?;
    
    if !response.status().is_success() {
        return Err(format!("GitHub returned status: {}", response.status()).into());
    }
    
    let content = response.text().await?;
    
    // Ensure the resources directory exists
    let resources_dir = Path::new("resources");
    if !resources_dir.exists() {
        fs::create_dir_all(resources_dir)?;
    }
    
    let target_path = resources_dir.join("fingerprints.json");
    fs::write(&target_path, content)?;
    
    Ok(())
}
