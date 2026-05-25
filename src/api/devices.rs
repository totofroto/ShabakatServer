use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    storage::devices as dev_store,
    AppState,
};

pub async fn list_devices(
    State(state): State<AppState>,
) -> impl IntoResponse {
    log::info!("[API] Fetching all devices from database");
    
    let mut db_rows = match dev_store::list_devices_async(state.db.clone(), false).await {
        Ok(rows) => rows,
        Err(e) => {
            log::error!("[API] Failed to list devices: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
        }
    };

    if db_rows.is_empty() {
        // Fallback to in-memory devices if DB is empty (e.g. first scan in progress)
        let mut mem_devices = state.devices.lock().unwrap().clone();
        if !mem_devices.is_empty() {
            log::info!("[API] DB empty, returning {} devices from memory", mem_devices.len());
            // Map DiscoveredDevice to a structure compatible with the frontend's expected format
            let now = crate::storage::now_ms();
            let mapped: Vec<serde_json::Value> = mem_devices.iter_mut().map(|d| {
                d.generate_suggested_names();
                
                json!({
                    "id": 0,
                    "mac": d.mac,
                    "lastIp": d.ip,
                    "vendor": d.vendor,
                    "vendorName": d.vendor_name,
                    "displayName": d.name,
                    "isOnline": true,
                    "lastSeen": now,
                    "firstSeen": now,
                    "likelyType": d.likely_type,
                    "hostname": d.hostname,
                    "mdnsHostname": d.mdns_hostname,
                    "ssdpServer": d.ssdp_server,
                    "acknowledged": false,
                    "suggestedNames": d.suggested_names,
                })
            }).collect();
            return Json(json!(mapped)).into_response();
        }
    }

    log::info!("[API] Returning {} devices from database", db_rows.len());
    for dev in &mut db_rows {
        dev.generate_suggested_names();
    }
    Json(json!(db_rows)).into_response()
}

pub async fn get_device(
    State(state): State<AppState>,
    Path(mac): Path<String>,
) -> impl IntoResponse {
    match dev_store::get_device_by_mac_async(state.db.clone(), mac).await {
        Ok(Some(mut row)) => {
            row.generate_suggested_names();
            Json(row).into_response()
        },
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

#[derive(Deserialize)]
pub struct DevicePatch {
    pub custom_name: Option<String>,
    pub notes: Option<String>,
    pub acknowledged: Option<bool>,
    pub custom_icon: Option<String>,
}

pub async fn patch_device(
    State(state): State<AppState>,
    Path(mac): Path<String>,
    Json(body): Json<DevicePatch>,
) -> impl IntoResponse {
    match dev_store::update_device_custom_fields(
        state.db.clone(),
        mac,
        body.custom_name,
        body.notes,
        body.acknowledged,
        body.custom_icon,
    )
    .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAliasRequest {
    pub ip_address: String,
    pub alias_name: String,
}

pub async fn set_device_alias(
    State(state): State<AppState>,
    Json(body): Json<DeviceAliasRequest>,
) -> impl IntoResponse {
    match dev_store::upsert_device_alias(state.db.clone(), body.ip_address, body.alias_name).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

pub async fn delete_device(
    State(state): State<AppState>,
    Path(mac): Path<String>,
) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    match dev_store::delete_device(&conn, &mac).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
