use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use tokio::{net::TcpStream, time::timeout};
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    catalog::fetch_nodes,
    model::{
        ConnectRequest, FavoriteRequest, GatewaySettings, Node, NodeStatus, NodeTestBatchRequest,
        NodeTestResult, RouteMode,
    },
    openvpn::{connect_node, stop_openvpn},
    store::{AppState, save_nodes, save_runtime, save_settings},
};

pub fn router(state: AppState) -> Router {
    let web_dir = state.cfg.web_dir.clone();
    Router::new()
        .route("/api/status", get(status))
        .route("/api/nodes", get(nodes))
        .route("/api/logs", get(logs))
        .route("/api/health", get(health))
        .route("/api/exit_ip", get(exit_ip))
        .route("/api/settings", get(settings).post(update_settings))
        .route("/api/favorite", post(toggle_favorite))
        .route("/api/test_node", post(test_node))
        .route("/api/test_nodes", post(test_nodes))
        .route("/api/refresh", post(refresh))
        .route("/api/autoconnect", post(autoconnect))
        .route("/api/connect", post(connect))
        .route("/api/disconnect", post(disconnect))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
        .fallback_service(ServeDir::new(web_dir).append_index_html_on_directories(true))
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    let runtime = state.runtime.read().await.clone();
    Json(runtime).into_response()
}

async fn nodes(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    let nodes = state.nodes.read().await.clone();
    Json(json!({ "nodes": nodes })).into_response()
}

async fn logs(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    let logs = state.logs.read().await.clone();
    Json(json!({ "logs": logs })).into_response()
}

async fn settings(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    let settings = state.settings.read().await.clone();
    Json(settings).into_response()
}

async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GatewaySettings>,
) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    {
        let mut guard = state.settings.write().await;
        *guard = payload.clone();
    }
    state
        .log("INFO", "settings", "routing settings updated")
        .await;
    match save_settings(&state.cfg, &payload).await {
        Ok(()) => Json(json!({ "ok": true, "settings": payload })).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn toggle_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FavoriteRequest>,
) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    let settings = {
        let mut guard = state.settings.write().await;
        if guard.favorite_node_ids.contains(&payload.id) {
            guard.favorite_node_ids.remove(&payload.id);
        } else {
            guard.favorite_node_ids.insert(payload.id);
        }
        guard.clone()
    };
    save_settings(&state.cfg, &settings).await.ok();
    state.log("INFO", "settings", "favorite list updated").await;
    Json(json!({ "ok": true, "settings": settings })).into_response()
}

async fn health(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    let relay_ok = TcpStream::connect(state.cfg.relay_addr).await.is_ok();
    let runtime = state.runtime.read().await.clone();
    Json(json!({
        "ok": relay_ok,
        "relay": {
            "addr": state.cfg.relay_addr.to_string(),
            "ok": relay_ok
        },
        "tunnel": {
            "device": state.cfg.tunnel_device,
            "active_node_id": runtime.active_node_id
        }
    }))
    .into_response()
}

async fn exit_ip(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    match check_exit_ip(&state).await {
        Ok((ip, latency_ms)) => {
            Json(json!({ "ok": true, "ip": ip, "latency_ms": latency_ms })).into_response()
        }
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "ok": false, "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn refresh(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    match do_refresh(&state).await {
        Ok(count) => Json(json!({ "ok": true, "count": count })).into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "ok": false, "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn test_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConnectRequest>,
) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    match do_test_node(&state, &payload.id).await {
        Some(result) => Json(json!({ "ok": true, "result": result })).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": "node not found" })),
        )
            .into_response(),
    }
}

async fn test_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<NodeTestBatchRequest>,
) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    let mut results = Vec::new();
    for id in payload.ids.iter().take(200) {
        if let Some(result) = do_test_node(&state, id).await {
            results.push(result);
        }
    }
    let available = results.iter().filter(|result| result.ok).count();
    Json(json!({
        "ok": true,
        "tested": results.len(),
        "available": available,
        "results": results
    }))
    .into_response()
}

async fn autoconnect(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    match select_node(&state).await {
        Some(node_id) => match connect_node(&state, &node_id).await {
            Ok(message) => {
                Json(json!({ "ok": true, "id": node_id, "message": message })).into_response()
            }
            Err(err) => (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "ok": false, "error": err.to_string() })),
            )
                .into_response(),
        },
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": "no route candidate matched current settings" })),
        )
            .into_response(),
    }
}

async fn connect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConnectRequest>,
) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    match connect_node(&state, &payload.id).await {
        Ok(message) => Json(json!({ "ok": true, "message": message })).into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "ok": false, "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn disconnect(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if authorize(&state, &headers).is_err() {
        return unauthorized_response();
    }
    match stop_openvpn(&state).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn do_refresh(state: &AppState) -> anyhow::Result<usize> {
    let mut fresh_nodes = fetch_nodes(&state.cfg).await?;
    let settings = state.settings.read().await.clone();
    for node in fresh_nodes.iter_mut() {
        if settings.favorite_node_ids.contains(&node.id) {
            node.status = NodeStatus::Available;
        }
    }
    let count = fresh_nodes.len();
    {
        let mut guard = state.nodes.write().await;
        *guard = fresh_nodes;
        save_nodes(&state.cfg, &guard).await?;
    }
    {
        let mut runtime = state.runtime.write().await;
        runtime.node_count = count;
        runtime.last_refresh_at = Some(unix_now());
        runtime.last_message = format!("refreshed {count} routes");
        save_runtime(&state.cfg, &runtime).await.ok();
    }
    state
        .log("INFO", "catalog", format!("refreshed {count} routes"))
        .await;
    Ok(count)
}

async fn do_test_node(state: &AppState, id: &str) -> Option<NodeTestResult> {
    let node = {
        let nodes = state.nodes.read().await;
        nodes.iter().find(|n| n.id == id).cloned()
    }?;
    let started = std::time::Instant::now();
    let target = format!("{}:{}", node.remote_host, node.remote_port);
    let result = timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(&target),
    )
    .await;
    let test_result = match result {
        Ok(Ok(_)) => NodeTestResult {
            id: id.to_string(),
            ok: true,
            latency_ms: Some(started.elapsed().as_millis()),
            message: "tcp reachable".to_string(),
        },
        Ok(Err(err)) => NodeTestResult {
            id: id.to_string(),
            ok: false,
            latency_ms: None,
            message: err.to_string(),
        },
        Err(_) => NodeTestResult {
            id: id.to_string(),
            ok: false,
            latency_ms: None,
            message: "tcp timeout".to_string(),
        },
    };

    {
        let mut nodes = state.nodes.write().await;
        if let Some(item) = nodes.iter_mut().find(|n| n.id == id) {
            item.latency_ms = test_result.latency_ms;
            item.status = if test_result.ok {
                NodeStatus::Available
            } else {
                NodeStatus::Failed
            };
            item.last_error = if test_result.ok {
                None
            } else {
                Some(test_result.message.clone())
            };
        }
        save_nodes(&state.cfg, &nodes).await.ok();
    }
    state
        .log(
            if test_result.ok { "INFO" } else { "WARNING" },
            "test",
            format!("{}: {}", test_result.id, test_result.message),
        )
        .await;
    Some(test_result)
}

async fn check_exit_ip(state: &AppState) -> anyhow::Result<(String, u128)> {
    let proxy_url = format!("socks5h://{}", state.cfg.relay_addr);
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy_url)?)
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let started = std::time::Instant::now();
    let ip = client
        .get("https://api.ipify.org")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let latency_ms = started.elapsed().as_millis();
    state
        .log("INFO", "health", format!("exit ip checked: {ip}"))
        .await;
    Ok((ip.trim().to_string(), latency_ms))
}

async fn select_node(state: &AppState) -> Option<String> {
    let settings = state.settings.read().await.clone();
    if !settings.connection_enabled {
        return None;
    }
    let nodes = state.nodes.read().await;
    let mut candidates: Vec<&Node> = nodes
        .iter()
        .filter(|n| !matches!(n.status, NodeStatus::Failed))
        .collect();

    match settings.route_mode {
        RouteMode::Auto => {}
        RouteMode::FixedCountry => {
            if !settings.country.is_empty() {
                candidates.retain(|n| {
                    n.country_short.eq_ignore_ascii_case(&settings.country)
                        || n.country.eq_ignore_ascii_case(&settings.country)
                });
            }
        }
        RouteMode::FixedNode => {
            candidates.retain(|n| n.id == settings.fixed_node_id);
        }
        RouteMode::Favorites => {
            let favs: Vec<&Node> = candidates
                .iter()
                .copied()
                .filter(|n| settings.favorite_node_ids.contains(&n.id))
                .collect();
            if !favs.is_empty() || !settings.fallback_to_any {
                candidates = favs;
            }
        }
    }

    candidates
        .into_iter()
        .min_by_key(|n| (n.ping <= 0, n.ping, -n.score))
        .map(|n| n.id.clone())
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected) = state.cfg.api_token.as_ref() else {
        return Ok(());
    };
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| headers.get("x-vgl-token").and_then(|v| v.to_str().ok()));

    if token == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn unauthorized_response() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "ok": false, "error": "unauthorized" })),
    )
        .into_response()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
