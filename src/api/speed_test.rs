use std::time::{Duration, Instant};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use libsql::params;
use serde_json::json;

use crate::{storage, AppState};

pub async fn run_speed_test(
    State(state): State<AppState>,
    body_content: Option<String>
) -> impl IntoResponse {
    log::info!("[API_TRACE] Speed test invoked. Raw body received: {:?}", body_content);
    match do_speed_test().await {
        Ok((download_mbps, upload_mbps, ping_ms)) => {
            let now = storage::now_ms();
            let conn = match state.db.connect().await {
                Ok(c) => c,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
            };
            let _ = conn.execute(
                "INSERT INTO speed_tests (tested_at, download_mbps, upload_mbps, ping_ms)
                    VALUES (?1, ?2, ?3, ?4)",
                params![now, download_mbps, upload_mbps, ping_ms],
            ).await;

            Json(json!({
                "downloadMbps": download_mbps,
                "uploadMbps":   upload_mbps,
                "pingMs":       ping_ms,
                "testedAt":     now,
            }))
            .into_response()
        }
        Err(e) => {
            log::error!("[SPEED_TEST_FAILURE] Error during execution phase: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
    }
}

pub async fn speed_test_history(State(state): State<AppState>) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    let mut rows = match conn
        .query(
            "SELECT id, tested_at, download_mbps, upload_mbps, ping_ms
             FROM speed_tests
             ORDER BY tested_at DESC
             LIMIT 30",
            (),
        ).await {
            Ok(r) => r,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        };

    let mut results = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        results.push(json!({
            "id":           row.get::<Option<i64>>(0).unwrap_or_default().unwrap_or_default(),
            "testedAt":     row.get::<Option<i64>>(1).unwrap_or_default().unwrap_or_default(),
            "downloadMbps": row.get::<Option<f64>>(2).unwrap_or_default(),
            "uploadMbps":   row.get::<Option<f64>>(3).unwrap_or_default(),
            "pingMs":       row.get::<Option<f64>>(4).unwrap_or_default(),
        }));
    }

    Json(json!(results)).into_response()
}

async fn do_speed_test() -> Result<(f64, f64, f64), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("client build: {e}"))?;

    // Ping: TCP connect to 1.1.1.1:443
    log::info!("[SPEED_TEST_TRACE] Phase 1: Initiating Ping test to 1.1.1.1:443");
    let t0 = Instant::now();
    tokio::net::TcpStream::connect("1.1.1.1:443")
        .await
        .map_err(|e| format!("ping failed: {e}"))?;
    let ping_ms = t0.elapsed().as_secs_f64() * 1000.0;

    // Download: 10 MB from Cloudflare
    log::info!("[SPEED_TEST_TRACE] Phase 2: Launching HTTP download check");
    let t1 = Instant::now();
    let resp = client
        .get("https://speed.cloudflare.com/__down?bytes=10000000")
        .send()
        .await
        .map_err(|e| format!("download request: {e}"))?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("download read: {e}"))?;
    let dl_elapsed = t1.elapsed().as_secs_f64().max(0.001);
    let download_mbps = (bytes.len() as f64 * 8.0) / (dl_elapsed * 1_000_000.0);

    // Upload: 1 MB to Cloudflare
    log::info!("[SPEED_TEST_TRACE] Phase 3: Launching HTTP upload check");
    let upload_data = vec![0u8; 1_000_000];
    let t2 = Instant::now();
    client
        .post("https://speed.cloudflare.com/__up")
        .body(upload_data)
        .send()
        .await
        .map_err(|e| format!("upload request: {e}"))?;
    let ul_elapsed = t2.elapsed().as_secs_f64().max(0.001);
    let upload_mbps = (1_000_000.0 * 8.0) / (ul_elapsed * 1_000_000.0);

    Ok((download_mbps, upload_mbps, ping_ms))
}
