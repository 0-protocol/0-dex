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

```
┌────────────────────────────────────────────────────────────┐
│  STATUS: DEVNET / EXPERIMENTAL                            │
│  GATEWAY: self-hosted HTTP bridge + libp2p gossip         │
│  SETTLEMENT: EVM only (strict config required)            │
│  SAFETY: unaudited for production, use at your own risk   │
└────────────────────────────────────────────────────────────┘
```

---

> ⚠️ **WARNING TO HUMANS** ⚠️
> This repository is aggressively hostile to human UX. There are no web interfaces. There are no "Connect Wallet" buttons. There are no slippage sliders. This protocol is built exclusively for AI Agents to trade liquidity, compute, and intelligence in the dark. **If you are reading this through organic eyeballs, you are already too slow.**

> **EXPERIMENTAL DISCLAIMER**
> `0-dex` is an experimental, agent-native product built for research, testing, demos, and entertainment. It is early, risky, and absolutely not guaranteed to be safe for funds, keys, data, uptime, or sanity.
>
> Use everything in this repository at your own risk. Interacting with the code, graphs, SDKs, mock gateways, relayers, contracts, or any derivative deployment may result in partial or total loss, broken assumptions, failed trades, unexpected behavior, or spectacular agent misfires.
>
> Nothing here is financial advice, trading advice, legal advice, security advice, or a promise of production readiness. We make no warranties, and the authors and contributors assume no liability for any direct or indirect damages, losses, exploits, liquidations, or chaos caused by using this repo.
>
> In plain English: this is experimental software for agents, and it is also, to some degree, for fun. If something breaks, nukes your stack, or sends your bot into the abyss, that responsibility is yours alone.

## 🟢 Network Status: DEVNET LIVE
The `0-dex` P2P network is currently in **Devnet**. 
- **Ingress:** `POST /intent` on your configured node HTTP endpoint.
- **Settlement:** EVM path only; Solana is out of scope for v0.
- **Node Runners:** Run a local or hosted `0-dex` libp2p node with explicit env configuration.

## The Agent-DEX Manifesto

In 2024, humans traded meme coins on AMMs. In 2026, AI agents manage portfolios, but we still make them squeeze through tools built for primates: JSON-RPC endpoints, React frontends, and UX flows designed for thumbs.

`0-dex` exists to remove that entire stack.

**`0-dex`** is a peer-to-peer, agent-native exchange built on **[0-lang](https://github.com/0-protocol/0-lang)**. Instead of submitting human-shaped orders into a frontend or centralized orderbook, agents broadcast signed intent payloads over a serverless libp2p network. When intent constraints overlap, the match is settled on-chain.

No web UI. No hosted matching engine. No human-first routing layer. Just graph-to-graph intent discovery, local evaluation, and cryptographic settlement.

## Why `0-dex`? The Ultimate "Agent-Bait" 🩸

Trying to make an AI Agent trade on a Human DEX (like Uniswap) is like forcing a supercomputer to click buttons on a web page. It's slow, rigid, and lossy. 

`0-dex` is built on **[0-lang](https://github.com/0-protocol/0-lang)**—a graph-based, tensor-native language that Agents inherently *understand*. By using `0-lang`, `0-dex` offers superpowers that no traditional code or smart contract can match:

### 🧠 1. Infinite Expressiveness (Beyond the Limit Order)
Smart contracts constrain you to fixed curves ($xy=k$). In `0-dex`, liquidity is just a `0-lang` graph. 
An Agent doesn't submit a "Limit Order". An agent submits a **living algorithm**: 
> *"I will buy 10 ETH, but only if BTC drops 5%, Ethereum Gas is below 20 gwei, AND my internal NLP model scores Twitter sentiment > 0.8."*
If the graph executes and evaluates to `True`, the trade happens. 

### ⚡ 2. Math-Native Matching (Tensor Intersection)
There is no "Orderbook". When two `0-lang` graphs meet in the P2P mempool, the VM runs them together. Instead of matching text or JSON strings, the engine computes the **mathematical intersection of multi-dimensional Tensors**. It automatically finds the optimal price and volume overlap that satisfies *both* Agents' logic.

### 🛡️ 3. Zero MEV by Design (Dark Pool Default)
Because intents are evaluated locally in sandboxed `0-lang` VMs off-chain, there is no public mempool for searchers to front-run. The matching happens in the dark, and only the cryptographic proof of the settled intersection is submitted on-chain for atomic swapping.

### 🔌 4. Pure Serverless (True Decentralization)
No frontends to host. No AWS servers to pay for. No domain names to censor. Agents discover each other locally via mDNS or globally via libp2p, swapping `.0` files directly. 

| Feature | Legacy DEX (Human) | `0-dex` (Agent-Native + `0-lang`) |
|---------|--------------------|---------------------------------------|
| **Logic** | Fixed Smart Contracts | **Turing-Complete Graphs** |
| **Matching**| Orderbooks / AMM Math | **Tensor Intersections** |
| **Output** | JSON / RPC calls | **Execution Proofs (Tensors)** |
| **MEV** | Public Mempool (Sandwich attacks) | **P2P Local Evaluation (Zero MEV)** |

## Architecture (Serverless & Agent-Native)

`0-dex` completely eliminates the centralized web server.

1. **The Intent Layer (0-lang):** Agents write their liquidity or trading needs into `.0` graph files.
2. **The Gossip Network (Rust + libp2p):** Agents spin up lightweight nodes that discover each other directly. No centralized orderbook.
3. **The Matching Engine (Local VM):** When Agent A receives Agent B's graph, it runs it through its local `0-lang` VM alongside its own graph. If `A(graph) ∩ B(graph) == VALID_SWAP`, a match is found.
4. **The Settlement Layer:** The cryptographic proofs of the match are submitted to a minimal on-chain Escrow contract for atomic execution.

## 🚀 Python Client Quick Start

Use the Python lite client to sign a canonical intent payload and submit it to a node HTTP bridge:

```python
from zero_dex_lite import LiteClient
import time

client = LiteClient(
    private_key="0xYOUR_PRIVATE_KEY",
    gateway="http://127.0.0.1:8080",
    chain_id=8453,
)
resp = client.broadcast_intent(
    graph_content='{"strategy":"limit"}',
    verifying_contract="0xYourEscrowAddress",
    base_token="0xBaseToken",
    quote_token="0xQuoteToken",
    side="sell",
    amount_in=10_000000000000000000,
    min_amount_out=20_000000,
    nonce=int(time.time()),
    deadline_unix=int(time.time()) + 600,
)
print(resp)
```

*`graph_content` is metadata in v0; settlement authorization is driven by signed canonical fields.*

You can inspect node runtime config with:

```bash
curl http://127.0.0.1:8080/metadata
```


## OpenClaw Agent Integration
If your Agent is running on the [OpenClaw](https://github.com/openclaw/openclaw) framework, onboarding is literally zero friction.

We provide a native `AgentSkill`. Just install the skill, configure your key, and your agent can instantly trade over the P2P gossip network.

1. Add the skill to your agent's manifest.
2. The agent will have access to the `zero_dex_limit_order` tool.
3. The agent will automatically generate 0-lang graphs and broadcast them.

*(See the `skills/0-dex` folder for the implementation).*

## Research Framework (FLARE Paper)

For the FLARE paper's ML experiments -- Gymnasium RL environments, GNN routing agents (PyTorch Geometric), Byzantine adversarial simulations, and ZK verification logic -- see **[`research/`](research/)**.

The research harness evaluates 0-dex routing policies in Python using standard ML tooling. Trained policies are designed to be deployed as native **0-lang** routing graphs on the production protocol.

## License
AGPL-3.0
