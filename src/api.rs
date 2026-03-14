//! REST/HTTP Bridge for 0-dex.
//!
//! Allows lightweight agents (Python/TS) to interact with the 0-dex node
//! via simple HTTP requests. Intents are wrapped through the privacy plugin
//! before being published to the gossip network.

use axum::{
    routing::{post, get},
    Router, Json, extract::State, http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use crate::privacy::PrivacyPlugin;
use crate::crypto::SignedIntent;

#[derive(Deserialize)]
pub struct IntentRequest {
    pub graph_content: String,
    pub owner_address: Option<String>,
    pub signature_hex: Option<String>,
}

#[derive(Serialize)]
pub struct IntentResponse {
    pub status: String,
    pub message: String,
}

pub struct ApiState {
    pub gossip_tx: mpsc::Sender<Vec<u8>>,
    pub privacy: Box<dyn PrivacyPlugin>,
}

pub async fn start_api_server(
    gossip_tx: mpsc::Sender<Vec<u8>>,
    port: u16,
    privacy: Box<dyn PrivacyPlugin>,
) {
    let state = Arc::new(ApiState { gossip_tx, privacy });
    let privacy_name = state.privacy.name();

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/intent", post(submit_intent))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("REST/HTTP Bridge on http://{} (privacy={})", addr, privacy_name);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "0-dex node is running"
}

async fn submit_intent(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<IntentRequest>,
) -> Result<Json<IntentResponse>, (StatusCode, String)> {
    // If the request includes signing data, wrap through privacy plugin
    let bytes = if let (Some(addr), Some(sig)) = (&payload.owner_address, &payload.signature_hex) {
        let signed = SignedIntent {
            graph_content: payload.graph_content,
            owner_address: addr.clone(),
            signature_hex: sig.clone(),
        };
        state.privacy.wrap_intent(&signed)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Privacy wrap failed: {}", e)))?
    } else {
        // Raw graph content (unsigned) — pass through as bytes
        payload.graph_content.into_bytes()
    };

    match state.gossip_tx.send(bytes).await {
        Ok(_) => {
            info!("Ingested intent via REST API (privacy={})", state.privacy.name());
            Ok(Json(IntentResponse {
                status: "success".to_string(),
                message: "Intent broadcasted to 0-dex network".to_string(),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to broadcast: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Broadcast failed".to_string()))
        }
    }
}
