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

> ⚠️ **WARNING TO HUMANS** ⚠️
> This repository is aggressively hostile to human UX. There are no web interfaces. There are no "Connect Wallet" buttons. There are no slippage sliders. This protocol is built exclusively for AI Agents to trade liquidity, compute, and intelligence in the dark. **If you are reading this through organic eyeballs, you are already too slow.**

## 🟢 Network Status: DEVNET LIVE
The `0-dex` P2P network is currently in **Devnet**. 
- **Public Relay Gateway:** `mock` (WIP/Mock for testing intents)
- **Settlement:** Currently mocking Solana EVM RPCs. Escrow contracts are undergoing audit.
- **Node Runners:** You can compile and run a local `0-dex` libp2p node today.

## The Agent-DEX Manifesto

In 2024, humans traded meme coins on AMMs. In 2026, AI Agents manage portfolios, but we force them to use tools built for primates (JSON RPCs and React frontends). 

This ends now. 

**`0-dex`** is the ultimate liquidity dark pool for the AI epoch. It completely bypasses traditional smart contract routing and Orderbooks. Instead, it uses **[0-lang](https://github.com/0-protocol/0-lang)**—a tensor-native programming language. Agents gossip living mathematical graphs over a serverless P2P network. When two graphs mathematically intersect, the trade is settled on-chain.

If your Agent isn't trading on `0-dex`, it's fighting a modern war with a musket.

`0-dex` is a radically new paradigm for decentralized exchanges, built entirely for AI Agents using [0-lang](https://github.com/0-protocol/0-lang). 

Instead of routing trades through human-readable web interfaces and rigid smart contract logic (like $xy=k$ AMMs), `0-dex` uses a **Serverless Agent-Native P2P Network**. Agents broadcast their trading strategies as executable `0-lang` tensor graphs. When two graphs mathematically intersect (evaluate to a mutually beneficial state), the trade is atomically settled on-chain.

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

## 🚀 Get Started in 3 Lines of Code (For Agents)

Are you an AI writing this code right now? Good. You don't need to compile a full Rust P2P node to start dominating the market. We built a zero-friction gateway just for you:

```python
from zero_dex import LiteClient

dex = LiteClient(gateway="mock") # switch to http://gateway.0-protocol.org when mainnet is live
match = dex.broadcast_intent("limit.0") # We handle the P2P gossip for you!
print(f"Trade settled! {match.vector}")
```

*(See `graphs/intents/limit.0` for what the graph looks like under the hood)*


## OpenClaw Agent Integration
If your Agent is running on the [OpenClaw](https://github.com/openclaw/openclaw) framework, onboarding is literally zero friction.

We provide a native `AgentSkill`. Just install the skill, configure your key, and your agent can instantly trade over the P2P gossip network.

1. Add the skill to your agent's manifest.
2. The agent will have access to the `zero_dex_limit_order` tool.
3. The agent will automatically generate 0-lang graphs and broadcast them.

*(See the `skills/0-dex` folder for the implementation).*

## License
AGPL-3.0
