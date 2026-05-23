use std::time::Duration;

use log::info;
use libsql::params;
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

            if let Ok(conn) = state.db.connect().await {
                let _ = conn.execute(
                    "INSERT INTO outages (started_at, ended_at, duration_ms) VALUES (?1, NULL, NULL)",
                    params![now],
                ).await;
            }

            if state.config.telegram_bot_token.is_some() && state.config.telegram_chat_id.is_some() {
                let msg = format!("🔴 <b>Internet down</b> — {} UTC", time_str);
                state.notifications.broadcast_text(&state.config, &msg).await;
            }
        } else if was_down && reachable {
            was_down = false;
            let time_str = format_hhmm_utc(now);

            let mut duration_ms: i64 = 0;
            if let Ok(conn) = state.db.connect().await {
                let rows = conn.query(
                    "SELECT started_at FROM outages WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
                    (),
                ).await.ok();
                
                let mut started_at: Option<i64> = None;
                if let Some(mut r) = rows {
                    if let Ok(Some(row)) = r.next().await {
                        started_at = row.get(0).ok();
                    }
                }

                if let Some(s_at) = started_at {
                    let dur = now - s_at;
                    let _ = conn.execute(
                        "UPDATE outages SET ended_at = ?1, duration_ms = ?2 WHERE ended_at IS NULL",
                        params![now, dur],
                    ).await;
                    duration_ms = dur;
                }
            };

            let mins = duration_ms / 60_000;
            info!("[OUTAGE_DETECTOR] Internet UP — outage lasted {mins}m");

            if state.config.telegram_bot_token.is_some() && state.config.telegram_chat_id.is_some() {
                let msg = format!("🟢 <b>Internet restored</b> — {} UTC ({} min outage)", time_str, mins);
                state.notifications.broadcast_text(&state.config, &msg).await;
            }
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
