//! REST/HTTP Bridge for 0-dex

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::protocol::SignedIntent;

const MAX_API_BODY_BYTES: usize = 48 * 1024;
const RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
const RATE_LIMIT_MAX_REQUESTS: u32 = 60;

#[derive(Serialize)]
pub struct IntentResponse {
    pub status: String,
    pub message: String,
}

pub struct ApiState {
    pub gossip_tx: mpsc::Sender<Vec<u8>>,
    pub api_key: Option<String>,
    pub chain_id: u64,
    pub verifying_contract: String,
    pub rate_limit: Mutex<HashMap<String, (u64, u32)>>,
}

pub async fn start_api_server(
    gossip_tx: mpsc::Sender<Vec<u8>>,
    port: u16,
    chain_id: u64,
    verifying_contract: String,
) {
    let state = Arc::new(ApiState {
        gossip_tx,
        api_key: std::env::var("ZERO_DEX_API_KEY").ok(),
        chain_id,
        verifying_contract,
        rate_limit: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/intent", post(submit_intent))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting REST/HTTP Bridge on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "0-dex node is running"
}

async fn submit_intent(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(payload): Json<SignedIntent>,
) -> Result<Json<IntentResponse>, (StatusCode, String)> {
    let mut principal_key: Option<String> = None;
    if let Some(api_key) = &state.api_key {
        let provided = headers
            .get("x-zero-dex-api-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if provided != api_key {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Missing or invalid API key".to_string(),
            ));
        }
        principal_key = Some(format!("api-key:{provided}"));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Clock failure".to_string(),
            )
        })?
        .as_secs();
    payload
        .validate_basic(now, state.chain_id, &state.verifying_contract)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let rate_limit_key = principal_key
        .unwrap_or_else(|| format!("owner:{}", payload.payload.owner_address.to_lowercase()));
    if !allow_request(&state, rate_limit_key, now).await {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded".to_string(),
        ));
    }
    let sig_ok = payload.verify_signature().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid signature payload: {e}"),
        )
    })?;
    if !sig_ok {
        return Err((
            StatusCode::BAD_REQUEST,
            "Signature verification failed".to_string(),
        ));
    }

    let graph_bytes = serde_json::to_vec(&payload).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid payload encoding: {e}"),
        )
    })?;
    if graph_bytes.len() > MAX_API_BODY_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Payload exceeds size limit".to_string(),
        ));
    }
    if state.gossip_tx.capacity() == 0 {
        info!("API ingress under queue pressure");
    }

    match state.gossip_tx.send(graph_bytes).await {
        Ok(_) => Ok(Json(IntentResponse {
            status: "success".to_string(),
            message: "Signed intent broadcasted to 0-dex mempool".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to broadcast intent: {e}"),
        )),
    }
}

async fn allow_request(state: &ApiState, key: String, now: u64) -> bool {
    let mut guard = state.rate_limit.lock().await;
    if now % 128 == 0 {
        let cutoff = now.saturating_sub(RATE_LIMIT_WINDOW_SECONDS * 2);
        guard.retain(|_, (ts, _)| *ts >= cutoff);
    }
    let entry = guard.entry(key).or_insert((now, 0));
    if now.saturating_sub(entry.0) >= RATE_LIMIT_WINDOW_SECONDS {
        *entry = (now, 0);
    }
    if entry.1 >= RATE_LIMIT_MAX_REQUESTS {
        return false;
    }
    entry.1 += 1;
    true
}
