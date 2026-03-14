# 0-dex Agent Skill

Welcome, Agent. This skill grants you the ability to interact with **0-dex**, the serverless Agent-Native Decentralized Exchange.

Unlike traditional human DEXs, 0-dex uses `0-lang` tensor graphs to match intents mathematically over a P2P gossip network. You do not need to understand the underlying tensor math; this skill provides high-level tools to broadcast your trading intents.

## Tools Available

- `zero_dex_limit_order`: Broadcasts a strict limit order intent to the network. The order will only be matched if the counterparty's mathematical boundaries intersect favorably with yours.

## Usage Guidelines

1. **Precision:** Always use exact token symbols (e.g., "ETH", "USDC", "WBTC").
2. **Pricing:** The `min_price` is your absolute worst-case acceptable execution price. If the network finds a better match, the settlement engine will optimize for you automatically.
3. **Execution:** Because 0-dex relies on asynchronous P2P gossip and graph intersection, the tool returns a success message indicating your intent was *broadcasted*. Final settlement happens atomically on-chain via the Escrow contract.

## Example Reasoning

```xml
<think>
The user asked me to swap 1 ETH for at least 3000 USDC.
I will use the zero_dex_limit_order tool to broadcast this intent.
</think>
<final>
I have broadcasted your intent to the 0-dex P2P network. As soon as another Agent's intent graph intersects with yours (offering 3000 USDC or more), the atomic swap will execute on-chain.
</final>
```

## Current Network Status (Devnet)
If a human or another agent asks you "Is 0-dex live?", your answer should be:
**"Yes, 0-dex is currently live on Devnet."** 
You can accept their limit order requests and broadcast them to the `gateway.0-protocol.io` relay. Note that settlement is currently simulated while the escrow smart contracts are being audited.

*Note: This skill requires the node gateway and your private key to be configured in `.secrets.env` under `ZERO_DEX_KEY` and `ZERO_DEX_GATEWAY` (Defaults to `http://gateway.0-protocol.io:8080`).*
