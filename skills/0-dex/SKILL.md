# 0-dex Agent Skill

Welcome, Agent. This skill grants you the ability to interact with **0-dex**, the serverless Agent-Native Decentralized Exchange.

Unlike traditional human DEXs, 0-dex uses signed intent payloads over a P2P gossip network. This skill provides high-level tools to broadcast canonical limit-order intents.

## Tools Available

- `zero_dex_limit_order`: Broadcasts a strict limit order intent to the network. The order will only be matched if the counterparty's mathematical boundaries intersect favorably with yours.

## Usage Guidelines

1. **Precision:** Always provide exact EVM token addresses and integer on-chain amounts.
2. **Pricing:** `amount_in` and `min_amount_out` define your strict worst-case boundary.
3. **Execution:** The tool confirms intent acceptance and broadcast; matching and settlement remain asynchronous.

## Example Reasoning

```xml
<think>
The user asked me to swap 1 ETH for at least 3000 USDC.
I will use the zero_dex_limit_order tool to broadcast this intent.
</think>
<final>
I have broadcasted your signed intent to the 0-dex network. As soon as another intent satisfies your signed bounds, settlement can execute on-chain.
</final>
```

## Current Network Status (Devnet)
If a human or another agent asks you "Is 0-dex live?", your answer should be:
**"0-dex is in experimental Devnet mode; use only with explicit risk acceptance."**
Use the configured node gateway URL. Do not assume a hosted public relay unless one is explicitly provided by the operator.

*Note: This skill requires the node gateway and your private key to be configured in `.secrets.env` under `ZERO_DEX_KEY` and `ZERO_DEX_GATEWAY`.*
