use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::{scanner, storage::networks as net_store, storage::devices as dev_store, AppState};

pub async fn list_networks(State(state): State<AppState>) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    };

    match net_store::list_networks(&conn).await {
        Ok(networks) => Json(json!(networks)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
            .into_response(),
    }
}

pub async fn get_topology(State(state): State<AppState>) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    };

    let devices = match dev_store::list_devices(&conn, false).await {
        Ok(d) => d,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    };

    let info = scanner::network_identity::get_current_network_info().await;
    let gateway_ip = info.gateway;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut gateway_mac = None;

    for dev in &devices {
        let mac = &dev.mac;
        let ip = dev.last_ip.as_deref();
        let is_gateway = gateway_ip.as_deref() == ip;
        if is_gateway {
            gateway_mac = Some(mac.to_string());
        }

        nodes.push(json!({
            "id": mac,
            "label": dev.display_name.as_deref().or(dev.hostname.as_deref()).unwrap_or(mac),
            "ip": ip,
            "isGateway": is_gateway,
            "isOnline": dev.is_online,
            "likelyType": dev.likely_type,
            "vendor": dev.vendor,
        }));
    }

    if let Some(gw_mac) = gateway_mac {
        for dev in &devices {
            let mac = &dev.mac;
            if mac != &gw_mac {
                edges.push(json!({
                    "source": gw_mac.clone(),
                    "target": mac,
                }));
            }
        }
    }

    Json(json!({
        "nodes": nodes,
        "edges": edges,
    })).into_response()
}

pub async fn get_network_info() -> impl IntoResponse {
    let info = scanner::network_identity::get_current_network_info().await;
    Json(json!({
        "ssid":    info.ssid,
        "bssid":   info.bssid,
        "gateway": info.gateway,
        "subnet":  info.subnet,
    }))
}
