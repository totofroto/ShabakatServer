use std::process::Command;
use tokio::task::spawn_blocking;

/// Execute a raw ICMP ping using the system's ping utility.
/// Returns the latency in milliseconds if successful.
pub async fn ping_device(ip: &str, count: u32, timeout_ms: u32) -> Result<f64, String> {
    let ip = ip.to_string();
    let count_str = count.to_string();
    let timeout_str = (timeout_ms / 1000).max(1).to_string(); // ping -W is usually in seconds on many systems, or ms on others. 
    // On Linux, -W is seconds. On some others it might be different.
    // Let's use 1 second as minimum for -W if timeout_ms is 500.

    spawn_blocking(move || {
        let output = Command::new("ping")
            .args(["-c", &count_str, "-W", &timeout_str, &ip])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse latency from output (e.g., "time=1.23 ms")
            if let Some(line) = stdout.lines().find(|l| l.contains("time=")) {
                if let Some(time_part) = line.split("time=").nth(1) {
                    if let Some(ms_str) = time_part.split_whitespace().next() {
                        if let Ok(ms) = ms_str.parse::<f64>() {
                            return Ok(ms);
                        }
                    }
                }
            }
            // Fallback if parsing fails but command succeeded
            Ok(0.0)
        } else {
            Err("Ping failed".to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
