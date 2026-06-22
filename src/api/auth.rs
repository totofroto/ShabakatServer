use axum::{response::IntoResponse, Json};

pub async fn me() -> impl IntoResponse {
    Json(serde_json::json!({
        "sub": "local-admin",
        "email": "admin@local",
        "role": "admin",
    }))
}
