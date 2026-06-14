use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    storage::devices as dev_store,
    AppState,
    api::error::{ApiResult, ApiError},
    scanner::arp::normalize_mac,
};

pub async fn list_devices(
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    log::info!("[API] Fetching all devices from database");
    
    let mut db_rows = dev_store::list_devices_async(state.db.clone(), false).await
        .map_err(ApiError::Internal)?;

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
            return Ok(Json(json!(mapped)).into_response());
        }
    }

    log::info!("[API] Returning {} devices from database", db_rows.len());
    for dev in &mut db_rows {
        dev.is_online = dev.is_online(); // Use the dynamic check
        dev.generate_suggested_names();
    }
    Ok(Json(json!(db_rows)).into_response())
}

pub async fn get_device(
    State(state): State<AppState>,
    Path(mac): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let normalized = normalize_mac(&mac);
    match dev_store::get_device_by_mac_async(state.db.clone(), normalized).await
        .map_err(ApiError::Internal)? {
        Some(mut row) => {
            row.is_online = row.is_online(); // Use the dynamic check
            row.generate_suggested_names();
            Ok(Json(row).into_response())
        },
        None => Err(ApiError::NotFound("Device not found".to_string())),
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
) -> ApiResult<impl IntoResponse> {
    let normalized = normalize_mac(&mac);
    dev_store::update_device_custom_fields(
        state.db.clone(),
        normalized,
        body.custom_name,
        body.notes,
        body.acknowledged,
        body.custom_icon,
    )
    .await
    .map_err(ApiError::Internal)?;
    
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeviceAliasRequest {
    pub ip_address: String,
    pub alias_name: String,
}

pub async fn set_device_alias(
    State(state): State<AppState>,
    Json(body): Json<DeviceAliasRequest>,
) -> ApiResult<impl IntoResponse> {
    dev_store::upsert_device_alias(state.db.clone(), body.ip_address, body.alias_name).await
        .map_err(ApiError::Internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeviceLearnRequest {
    pub mac_address: String,
    pub name: String,
}

pub async fn learn_device(
    State(state): State<AppState>,
    Json(body): Json<DeviceLearnRequest>,
) -> ApiResult<impl IntoResponse> {
    let normalized = normalize_mac(&body.mac_address);
    log::info!("[API] Learning device: {} -> {}", normalized, body.name);

    state.db.execute(move |conn| {
        conn.execute(
            "UPDATE devices SET acknowledged = 1, custom_name = ?1 WHERE mac = ?2",
            [body.name, normalized],
        )
    }).await.map_err(ApiError::from)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeviceIgnoreRequest {
    pub mac_address: String,
}

pub async fn ignore_device(
    State(state): State<AppState>,
    Json(body): Json<DeviceIgnoreRequest>,
) -> ApiResult<impl IntoResponse> {
    let normalized = normalize_mac(&body.mac_address);
    log::info!("[API] Ignoring device: {}", normalized);

    dev_store::ignore_device_async(state.db.clone(), normalized).await
        .map_err(ApiError::Internal)?;
    
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn delete_device(
    State(state): State<AppState>,
    Path(mac): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let normalized = normalize_mac(&mac);
    let deleted = state.db.execute(move |conn| {
        dev_store::delete_device(conn, &normalized)
    }).await.map_err(ApiError::from)?;

    if deleted {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Device not found".to_string()))
    }
}

pub async fn get_device_rssi(
    State(state): State<AppState>,
    Path(mac): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let now = crate::storage::now_ms();
    let twenty_four_hours_ago = now - (24 * 60 * 60 * 1000);

    let history = state.db.execute(move |conn| {
        crate::storage::history::get_device_rssi_history(conn, &mac, twenty_four_hours_ago)
    }).await.map_err(ApiError::Internal)?;

    let mapped: Vec<serde_json::Value> = history.into_iter().map(|(ts, rssi)| {
        json!({
            "timestamp": ts,
            "rssi": rssi,
        })
    }).collect();
    
    Ok(Json(mapped))
}

