use std::time::Duration;

use log::info;
use tokio::net::TcpStream;

use crate::{storage, AppState};

pub async fn start_outage_monitor(state: AppState) {
    info!("[OUTAGE_DETECTOR] Starting internet outage monitor");
    let mut was_down = false;

    loop {
        let reachable = is_internet_reachable().await;
        let now = storage::now_ms();

        if !was_down && !reachable {
            was_down = true;
            let time_str = format_hhmm_utc(now);
            info!("[OUTAGE_DETECTOR] Internet DOWN at {time_str} UTC");

            let _ = state.db.execute(move |conn| {
                conn.execute(
                    "INSERT INTO outages (started_at, ended_at, duration_ms) VALUES (?1, NULL, NULL)",
                    rusqlite::params![now],
                )
            }).await;

            // 1. Log the outage into the unified historical event registry
            crate::storage::history::log_event(
                &state.db, 
                "outage", 
                None, 
                "WAN gateway connectivity failure detected."
            ).await;

            let msg = format!("🔴 <b>Internet down</b> — {} UTC", time_str);
            state.notifications.broadcast_alert(&state.db, "Network Outage", &msg).await;
        } else if was_down && reachable {
            was_down = false;
            let time_str = format_hhmm_utc(now);

            let res: Result<i64, String> = state.db.execute(move |conn| -> Result<i64, String> {
                let started_at: Option<i64> = conn.query_row(
                    "SELECT started_at FROM outages WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
                    [],
                    |row| row.get(0)
                ).ok();

                if let Some(s_at) = started_at {
                    let dur = now - s_at;
                    conn.execute(
                        "UPDATE outages SET ended_at = ?1, duration_ms = ?2 WHERE ended_at IS NULL",
                        rusqlite::params![now, dur],
                    ).map_err(|e| e.to_string())?;
                    Ok(dur)
                } else {
                    Ok(0)
                }
            }).await;

            let duration_ms = res.unwrap_or(0);
            let mins = duration_ms / 60_000;
            info!("[OUTAGE_DETECTOR] Internet UP — outage lasted {mins}m");

            // 1. Log the restoration into the unified historical event registry
            crate::storage::history::log_event(
                &state.db, 
                "restored", 
                None, 
                &format!("WAN gateway connectivity restored. Total downtime: {} minutes.", mins)
            ).await;

            let msg = format!("🟢 <b>Internet restored</b> — {} UTC ({} min outage)", time_str, mins);
            state.notifications.broadcast_alert(&state.db, "Network Restored", &msg).await;
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}


async fn is_internet_reachable() -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(5), TcpStream::connect("8.8.8.8:53")).await,
        Ok(Ok(_))
    )
}

fn format_hhmm_utc(ts_ms: i64) -> String {
    let secs = (ts_ms / 1000) as u64;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    format!("{hours:02}:{mins:02}")
}
