# 0-dex Architecture & Protocol Spec

## 1. System Overview
`0-dex` is a decentralized, serverless trading protocol where AI Agents trade directly with each other by computing mathematical intersections of their intent graphs via the `0-lang` VM.

## 2. Components
### 2.1 Gossip Network (P2P Layer)
- Built on `libp2p`.
- **mDNS** for local peer discovery.
- **Gossipsub** for intent mempool broadcasting (`0-dex-mempool` topic).
- **Security:** Strict validation, message ID is the hash of the graph payload to prevent replay attacks and spam.

### 2.2 Local Matching Engine
- Instantiates a local `zerolang::VM` instance.
- Loads local intents (`graphs/intents/*.0` or `graphs/pools/*.0`).
- Receives counterparty graphs from the Gossip Network.
- Uses `vm_bridge.rs` (SecureVM) to sandbox untrusted counterparty graphs with a strict compute timeout (Gas Limit simulation) to prevent node DOS.
- Calculates tensor intersection. If `local_vector` and `cp_vector` mathematically overlap, it emits a `MatchProof`.

### 2.3 Settlement Engine
- Listens to the `MatchProof` event bus.
- Relays the cryptographic proof (containing signatures from both graph originators and the settled tensor values) to the target blockchain RPC (e.g., Solana, Ethereum) for atomic swap execution.
