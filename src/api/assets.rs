use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::path::Path;
use tokio::fs;

use crate::AppState;

pub async fn upload_asset(
    State(_state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_path = String::new();

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or_default().to_string();
        let name_attr = field.file_name().unwrap_or_default().to_string();

        if name == "file" {
            let data = field.bytes().await.unwrap_or_default();
            
            // Limit size to 2MB
            if data.len() > 2 * 1024 * 1024 {
                return (StatusCode::PAYLOAD_TOO_LARGE, "File too large").into_response();
            }

            // Validate image signature (PNG/JPG)
            if !is_image(&data) {
                return (StatusCode::BAD_REQUEST, "Only PNG and JPG images are allowed").into_response();
            }

            let ext = Path::new(&name_attr)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("bin");
            
            let file_name = format!("{}.{}", uuid::Uuid::new_v4(), ext);
            let dest = format!("data/assets/{}", file_name);
            
            if let Err(e) = fs::write(&dest, data).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save file: {}", e)).into_response();
            }
            
            file_path = format!("/uploads/{}", file_name);
        }
    }

    if file_path.is_empty() {
        return (StatusCode::BAD_REQUEST, "No file uploaded").into_response();
    }

    (StatusCode::OK, Json(json!({ "url": file_path }))).into_response()
}

fn is_image(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    
    // PNG: 89 50 4E 47
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return true;
    }
    
    // JPEG: FF D8 FF
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return true;
    }
    
    false
}
