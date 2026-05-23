use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::AppState;

/// Handles multipart file upload for assets.
/// Specifically saves the uploaded file as 'blueprint.jpg' in the assets directory.
pub async fn upload_asset(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Ensure assets directory exists within data_dir
    let assets_dir = format!("{}/assets", state.config.data_dir);
    if let Err(e) = std::fs::create_dir_all(&assets_dir) {
        log::error!("Failed to create assets directory '{}': {}", assets_dir, e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to initialize storage" })),
        )
            .into_response();
    }

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();

        if name == "file" {
            let dest_path = Path::new(&assets_dir).join("blueprint.jpg");
            
            // Open file for writing
            let mut file = match fs::File::create(&dest_path).await {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Failed to create file {:?}: {}", dest_path, e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "Failed to create destination file" })),
                    )
                        .into_response();
                }
            };

            // Stream the field content into the file
            while let Ok(Some(chunk)) = field.chunk().await {
                if let Err(e) = file.write_all(&chunk).await {
                    log::error!("Error writing to file {:?}: {}", dest_path, e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "Failed to save file data" })),
                    )
                        .into_response();
                }
            }

            if let Err(e) = file.flush().await {
                log::error!("Error flushing file {:?}: {}", dest_path, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to finalize file" })),
                )
                    .into_response();
            }

            log::info!("Successfully uploaded asset to {:?}", dest_path);
            return (
                StatusCode::OK,
                Json(json!({ 
                    "status": "success",
                    "url": "/uploads/blueprint.jpg"
                })),
            )
                .into_response();
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": "No 'file' field found in multipart form" })),
    )
        .into_response()
}
