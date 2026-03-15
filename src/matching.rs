//! Deterministic order matching engine.

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use ethers::types::U256;
use tokio::sync::mpsc;

use crate::protocol::{compute_match_id, MatchProof, OrderSide, SignedIntent};

pub struct MatchingEngine {
    local_intents: Vec<SignedIntent>,
    match_sender: mpsc::Sender<MatchProof>,
    max_pool_size: usize,
}

impl MatchingEngine {
    pub fn new(match_sender: mpsc::Sender<MatchProof>) -> Self {
        Self {
            local_intents: Vec::new(),
            match_sender,
            max_pool_size: 10_000,
        }
    }

    pub async fn process_incoming_intent(&mut self, incoming: SignedIntent) -> bool {
        let now = now_unix();
        if incoming.payload.deadline_unix < now {
            tracing::warn!("Dropping expired intent");
            return false;
        }
        self.evict_expired(now);
        self.evict_duplicate_nonce(&incoming);

        for idx in 0..self.local_intents.len() {
            if let Some((amount_a, amount_b)) = compute_fill(&self.local_intents[idx], &incoming) {
                let existing = self.local_intents.swap_remove(idx);
                let match_id = compute_match_id(&existing, &incoming, amount_a, amount_b);
                let proof = MatchProof {
                    match_id,
                    maker_intent: existing,
                    taker_intent: incoming.clone(),
                    amount_a,
                    amount_b,
                    matched_at_unix: now,
                    relayer: std::env::var("ZERO_DEX_RELAYER_ADDRESS").ok(),
                };
                let _ = self.match_sender.send(proof).await;
                return true;
            }
        }

        if self.local_intents.len() >= self.max_pool_size {
            self.local_intents.remove(0);
        }
        self.local_intents.push(incoming);
        false
    }

    fn evict_expired(&mut self, now: u64) {
        self.local_intents
            .retain(|intent| intent.payload.deadline_unix >= now);
    }

    fn evict_duplicate_nonce(&mut self, incoming: &SignedIntent) {
        self.local_intents.retain(|intent| {
            !(intent
                .payload
                .owner_address
                .eq_ignore_ascii_case(&incoming.payload.owner_address)
                && intent.payload.nonce == incoming.payload.nonce)
        });
    }
}

fn compute_fill(a: &SignedIntent, b: &SignedIntent) -> Option<(u128, u128)> {
    if a.payload
        .owner_address
        .eq_ignore_ascii_case(&b.payload.owner_address)
    {
        return None;
    }
    if !is_non_zero_address(&a.payload.owner_address)
        || !is_non_zero_address(&b.payload.owner_address)
        || !is_non_zero_address(&a.payload.base_token)
        || !is_non_zero_address(&a.payload.quote_token)
        || !is_non_zero_address(&b.payload.base_token)
        || !is_non_zero_address(&b.payload.quote_token)
    {
        return None;
    }
    if a.payload.chain_id != b.payload.chain_id {
        return None;
    }
    if a.payload.base_token.to_lowercase() != b.payload.base_token.to_lowercase() {
        return None;
    }
    if a.payload.quote_token.to_lowercase() != b.payload.quote_token.to_lowercase() {
        return None;
    }
    if a.payload.side == b.payload.side {
        return None;
    }

    let (sell, buy, sell_is_a) = if a.payload.side == OrderSide::Sell {
        (a, b, true)
    } else {
        (b, a, false)
    };

    // Cross-multiply to avoid precision loss:
    // buy.price >= sell.price
    // buy.amount_in / buy.min_amount_out >= sell.min_amount_out / sell.amount_in
    // <=> sell.amount_in * buy.amount_in >= sell.min_amount_out * buy.min_amount_out
    let lhs = U256::from(sell.payload.amount_in) * U256::from(buy.payload.amount_in);
    let rhs = U256::from(sell.payload.min_amount_out) * U256::from(buy.payload.min_amount_out);
    if lhs < rhs {
        return None;
    }

    // v0 mainnet scope: single fill only (no partial fills).
    let amount_base = sell.payload.amount_in;
    let amount_quote = sell.payload.min_amount_out;
    if amount_base == 0 || amount_quote == 0 {
        return None;
    }
    if amount_base < buy.payload.min_amount_out {
        return None;
    }
    if amount_quote > buy.payload.amount_in {
        return None;
    }

    if sell_is_a {
        Some((amount_base, amount_quote))
    } else {
        // `a` is the buy order, so `a` sends quote and `b` sends base.
        Some((amount_quote, amount_base))
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn is_non_zero_address(raw: &str) -> bool {
    let Ok(addr) = ethabi::Address::from_str(raw) else {
        return false;
    };
    addr != ethabi::Address::zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::PROTOCOL_VERSION;

    fn signed(owner: &str, side: OrderSide, amount_in: u128, min_out: u128) -> SignedIntent {
        SignedIntent {
            payload: crate::protocol::IntentPayload {
                version: PROTOCOL_VERSION.to_string(),
                chain_id: 1,
                nonce: 1,
                deadline_unix: now_unix() + 60,
                owner_address: owner.to_string(),
                verifying_contract: "0x4444444444444444444444444444444444444444".to_string(),
                base_token: "0x2222222222222222222222222222222222222222".to_string(),
                quote_token: "0x3333333333333333333333333333333333333333".to_string(),
                side,
                amount_in,
                min_amount_out: min_out,
                graph_content: "graph".to_string(),
            },
            signature_hex: "0x".to_string(),
        }
    }

    #[test]
    fn matches_when_prices_overlap() {
        let sell = signed(
            "0x1111111111111111111111111111111111111111",
            OrderSide::Sell,
            100,
            180,
        );
        let buy = signed(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            OrderSide::Buy,
            200,
            100,
        );
        assert!(compute_fill(&sell, &buy).is_some());
    }

    #[test]
    fn computes_amounts_in_intent_token_in_units() {
        let sell = signed(
            "0x1111111111111111111111111111111111111111",
            OrderSide::Sell,
            100,
            180,
        );
        let buy = signed(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            OrderSide::Buy,
            200,
            100,
        );
        let (a_amt, b_amt) = compute_fill(&sell, &buy).expect("sell-first fill");
        assert_eq!(a_amt, 100);
        assert_eq!(b_amt, 180);

        let (a_amt_rev, b_amt_rev) = compute_fill(&buy, &sell).expect("buy-first fill");
        assert_eq!(a_amt_rev, 180); // buy order sends quote token
        assert_eq!(b_amt_rev, 100); // sell order sends base token
    }
}
