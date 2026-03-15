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

use crate::protocol::{SignedIntent, PROTOCOL_VERSION};

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
    pub local_intent_tx: mpsc::Sender<SignedIntent>,
    pub api_key: Option<String>,
    pub chain_id: u64,
    pub verifying_contract: String,
    pub settlement_mode: String,
    pub rate_limit: Mutex<HashMap<String, (u64, u32)>>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Serialize)]
pub struct MetadataResponse {
    pub protocol: String,
    pub chain_id: u64,
    pub verifying_contract: String,
    pub api_key_required: bool,
    pub settlement_mode: String,
}

pub async fn start_api_server(
    gossip_tx: mpsc::Sender<Vec<u8>>,
    local_intent_tx: mpsc::Sender<SignedIntent>,
    port: u16,
    chain_id: u64,
    verifying_contract: String,
    settlement_mode: String,
) {
    let state = Arc::new(ApiState {
        gossip_tx,
        local_intent_tx,
        api_key: std::env::var("ZERO_DEX_API_KEY").ok(),
        chain_id,
        verifying_contract,
        settlement_mode,
        rate_limit: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/metadata", get(metadata))
        .route("/intent", post(submit_intent))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting REST/HTTP Bridge on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn metadata(State(state): State<Arc<ApiState>>) -> Json<MetadataResponse> {
    Json(MetadataResponse {
        protocol: PROTOCOL_VERSION.to_string(),
        chain_id: state.chain_id,
        verifying_contract: state.verifying_contract.clone(),
        api_key_required: state.api_key.is_some(),
        settlement_mode: state.settlement_mode.clone(),
    })
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

    if let Err(e) = state.local_intent_tx.send(payload.clone()).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed local ingest enqueue: {e}"),
        ));
    }

    match state.gossip_tx.send(graph_bytes).await {
        Ok(_) => Ok(Json(IntentResponse {
            status: "success".to_string(),
            message: "Signed intent accepted, ingested locally, and broadcasted".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to broadcast intent: {e}"),
        )),
    }
}

async fn allow_request(state: &ApiState, key: String, now: u64) -> bool {
    let mut guard = state.rate_limit.lock().await;
    if now.is_multiple_of(128) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi;
    use crate::matching::MatchingEngine;
    use crate::protocol::{IntentPayload, OrderSide, SignedIntent, PROTOCOL_VERSION};
    use ethers::signers::{LocalWallet, Signer};
    use tokio::sync::mpsc;

    fn signed_intent(
        wallet: &LocalWallet,
        side: OrderSide,
        nonce: u64,
        amount_in: u128,
        min_amount_out: u128,
    ) -> SignedIntent {
        let payload = IntentPayload {
            version: PROTOCOL_VERSION.to_string(),
            chain_id: 1,
            nonce,
            deadline_unix: 4_102_444_800,
            owner_address: format!("{:?}", wallet.address()),
            verifying_contract: "0x4444444444444444444444444444444444444444".to_string(),
            base_token: "0x1111111111111111111111111111111111111111".to_string(),
            quote_token: "0x2222222222222222222222222222222222222222".to_string(),
            side,
            amount_in,
            min_amount_out,
            graph_content: "{\"strategy\":\"limit\"}".to_string(),
        };
        let unsigned = SignedIntent {
            payload: payload.clone(),
            signature_hex: String::new(),
        };
        let digest = unsigned.eip712_digest().expect("digest");
        let signature = wallet.sign_hash(digest.into()).expect("sign");
        SignedIntent {
            payload,
            signature_hex: format!("0x{}", signature),
        }
    }

    #[tokio::test]
    async fn api_ingress_to_match_to_abi_smoke() {
        let (gossip_tx, mut gossip_rx) = mpsc::channel::<Vec<u8>>(8);
        let (local_tx, mut local_rx) = mpsc::channel::<SignedIntent>(8);
        let state = Arc::new(ApiState {
            gossip_tx,
            local_intent_tx: local_tx,
            api_key: None,
            chain_id: 1,
            verifying_contract: "0x4444444444444444444444444444444444444444".to_string(),
            settlement_mode: "simulation".to_string(),
            rate_limit: Mutex::new(HashMap::new()),
        });
        let wallet_a: LocalWallet =
            "0x59c6995e998f97a5a0044976f8f2b8d2f22ebf0c6f0f4f7f3afccf4d7ed2d1a5"
                .parse()
                .expect("wallet_a");
        let wallet_b: LocalWallet =
            "0x8b3a350cf5c34c9194ca3a545d15f6d4f0d90a2f9f0f2b8f4e9d86f6b4f5a3e2"
                .parse()
                .expect("wallet_b");

        let intent_sell = signed_intent(&wallet_a, OrderSide::Sell, 1, 100, 200);
        let intent_buy = signed_intent(&wallet_b, OrderSide::Buy, 1, 220, 100);

        let headers = HeaderMap::new();
        let _ = submit_intent(State(state.clone()), headers.clone(), Json(intent_sell))
            .await
            .expect("submit sell");
        let _ = submit_intent(State(state), headers, Json(intent_buy))
            .await
            .expect("submit buy");

        let _gossip_1 = gossip_rx.recv().await.expect("gossip payload 1");
        let _gossip_2 = gossip_rx.recv().await.expect("gossip payload 2");

        let (match_tx, mut match_rx) = mpsc::channel(4);
        let mut matching = MatchingEngine::new(match_tx);
        let local_1 = local_rx.recv().await.expect("local intent 1");
        let local_2 = local_rx.recv().await.expect("local intent 2");
        let _ = matching.process_incoming_intent(local_1).await;
        let matched = matching.process_incoming_intent(local_2).await;
        assert!(matched);

        let proof = match_rx.recv().await.expect("match proof");
        let encoded = abi::encode_match_for_evm(&proof).expect("abi encode");
        assert!(encoded.len() > 4);
    }
}
