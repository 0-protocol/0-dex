//! REST/HTTP Bridge for 0-dex
//! 
//! Allows lightweight agents (Python/TS) to interact with the 0-dex node
//! via simple HTTP requests without needing to implement libp2p or 0-lang locally.

use axum::{
    routing::{post, get},
    Router, Json, extract::State, http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

#[derive(Deserialize)]
pub struct IntentRequest {
    /// The raw .0 graph content as a string
    pub graph_content: String,
}

#[derive(Serialize)]
pub struct IntentResponse {
    pub status: String,
    pub message: String,
}

pub struct ApiState {
    pub gossip_tx: mpsc::Sender<Vec<u8>>,
}

/// Start the REST API server
pub async fn start_api_server(gossip_tx: mpsc::Sender<Vec<u8>>, port: u16) {
    let state = Arc::new(ApiState { gossip_tx });

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
    Json(payload): Json<IntentRequest>,
) -> Result<Json<IntentResponse>, (StatusCode, String)> {
    let graph_bytes = payload.graph_content.into_bytes();
    
    // Send to the gossip network to be broadcasted
    match state.gossip_tx.send(graph_bytes).await {
        Ok(_) => {
            info!("Successfully ingested external intent via REST API");
            Ok(Json(IntentResponse {
                status: "success".to_string(),
                message: "Intent broadcasted to 0-dex mempool".to_string(),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to broadcast intent: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to broadcast intent".to_string(),
            ))
        }
    }
}
