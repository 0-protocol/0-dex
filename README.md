<div align="center">

```
 ██████╗       ██████╗ ███████╗██╗  ██╗
██╔═████╗      ██╔══██╗██╔════╝╚██╗██╔╝
██║██╔██║█████╗██║  ██║█████╗   ╚███╔╝ 
████╔╝██║╚════╝██║  ██║██╔══╝   ██╔██╗ 
╚██████╔╝      ██████╔╝███████╗██╔╝ ██╗
 ╚═════╝       ╚═════╝ ╚══════╝╚═╝  ╚═╝
```

### **The First Agent-Native Decentralized Exchange.**

[![License](https://img.shields.io/badge/License-AGPL_3.0-blue.svg)](LICENSE)
[![0-lang](https://img.shields.io/badge/Built_With-0--lang-black.svg)](https://github.com/0-protocol/0-lang)

---

*Zero UI. Zero Middlemen. Pure Graph-to-Graph Intent Matching.*

</div>

---

## What is `0-dex`?

`0-dex` is a radically new paradigm for decentralized exchanges, built entirely for AI Agents using [0-lang](https://github.com/0-protocol/0-lang). 

Instead of routing trades through human-readable web interfaces and rigid smart contract logic (like $xy=k$ AMMs), `0-dex` uses a **Serverless Agent-Native P2P Network**. Agents broadcast their trading strategies as executable `0-lang` tensor graphs. When two graphs mathematically intersect (evaluate to a mutually beneficial state), the trade is atomically settled on-chain.

## Why Build a DEX exclusively for Agents?

Humans and Agents trade differently. Trying to make an Agent use a Human DEX is like making an API click buttons on a screen.

| Feature | Human DEXs (Uniswap, 1inch) | `0-dex` (Agent-Native) |
|---------|----------------------------|------------------------|
| **Liquidity Definition** | Fixed curves (Constant Product, Concentrated) | **Turing-Complete Graphs** (Liquidity is a 0-lang executable algorithm) |
| **Order Types** | Market, Limit, TWAP | **Infinite Intents** ("Buy ETH if BTC moves 5% AND network gas < 20") |
| **Hosting** | Centralized Frontend + RPC Nodes | **Fully Serverless/P2P** (Agents gossip graphs directly via `libp2p`) |
| **MEV Protection** | Sandwiched by bots | **Cryptographic Graph Matching** (Matches happen off-chain in private environments, settlement is atomic) |
| **Communication** | JSON over HTTP/WebSocket | **Native Tensors** (0-lang handles dimensions, probabilities, and execution bounds) |

## Architecture (Serverless & Agent-Native)

`0-dex` completely eliminates the centralized web server.

1. **The Intent Layer (0-lang):** Agents write their liquidity or trading needs into `.0` graph files.
2. **The Gossip Network (Rust + libp2p):** Agents spin up lightweight nodes that discover each other directly. No centralized orderbook.
3. **The Matching Engine (Local VM):** When Agent A receives Agent B's graph, it runs it through its local `0-lang` VM alongside its own graph. If `A(graph) ∩ B(graph) == VALID_SWAP`, a match is found.
4. **The Settlement Layer:** The cryptographic proofs of the match are submitted to a minimal on-chain Escrow contract for atomic execution.

## Example: A Limit Order in 0-lang

*(See `graphs/intents/limit.0`)*
Instead of sending a JSON payload to a server, the agent broadcasts an executable graph that evaluates price conditions locally on counterparties' machines.

## License
AGPL-3.0
