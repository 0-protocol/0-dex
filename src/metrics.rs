use prometheus::{IntCounter, Registry, TextEncoder, Encoder, register_int_counter_with_registry};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    
    pub static ref INTENTS_RECEIVED: IntCounter = 
        register_int_counter_with_registry!(
            "zero_dex_intents_received_total",
            "Total number of intent graphs received via gossip",
            REGISTRY
        ).unwrap();

    pub static ref INTENTS_PUBLISHED: IntCounter = 
        register_int_counter_with_registry!(
            "zero_dex_intents_published_total",
            "Total number of intent graphs published locally",
            REGISTRY
        ).unwrap();

    pub static ref MATCHES_FOUND: IntCounter = 
        register_int_counter_with_registry!(
            "zero_dex_matches_found_total",
            "Total number of cross-intent matches found",
            REGISTRY
        ).unwrap();
        
    pub static ref TRANSACTIONS_SUBMITTED: IntCounter = 
        register_int_counter_with_registry!(
            "zero_dex_tx_submitted_total",
            "Total number of settlement transactions submitted on-chain",
            REGISTRY
        ).unwrap();
}

pub fn encode_metrics() -> String {
    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
