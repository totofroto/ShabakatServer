use axum::{
    extract::{Multipart, State},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::api::error::{ApiError, ApiResult};
use crate::AppState;

/// Handles multipart file upload for assets.
/// Specifically saves the uploaded file as 'blueprint.jpg' in the assets directory.
pub async fn upload_asset(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<impl IntoResponse> {
    // Ensure assets directory exists within data_dir
    let assets_dir = format!("{}/assets", state.config.data_dir);
    if let Err(e) = std::fs::create_dir_all(&assets_dir) {
        log::error!("Failed to create assets directory '{}': {}", assets_dir, e);
        return Err(ApiError::Internal("Failed to initialize storage".to_string()));
    }

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();

        if name == "file" {
            let dest_path = Path::new(&assets_dir).join("blueprint.jpg");

            // Open file for writing
            let mut file = fs::File::create(&dest_path).await.map_err(|e| {
                log::error!("Failed to create file {:?}: {}", dest_path, e);
                ApiError::Internal("Failed to create destination file".to_string())
            })?;

            // Stream the field content into the file
            while let Ok(Some(chunk)) = field.chunk().await {
                file.write_all(&chunk).await.map_err(|e| {
                    log::error!("Error writing to file {:?}: {}", dest_path, e);
                    ApiError::Internal("Failed to save file data".to_string())
                })?;
            }

            file.flush().await.map_err(|e| {
                log::error!("Error flushing file {:?}: {}", dest_path, e);
                ApiError::Internal("Failed to finalize file".to_string())
            })?;

            log::info!("Successfully uploaded asset to {:?}", dest_path);
            return Ok(Json(json!({
                "status": "success",
                "url": "/uploads/blueprint.jpg"
            })));
        }
    }

    Err(ApiError::BadRequest(
        "No 'file' field found in multipart form".to_string(),
    ))
}
