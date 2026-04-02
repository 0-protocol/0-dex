"""GATv2-based routing policy network for dark-pool order splitting.

Architecture
------------
The maker federation is modelled as a dynamic graph where each maker is a
node.  A 2-layer GATv2Conv backbone with multi-head attention learns
**which makers to trust** based on their reported liquidity, historical fill
rates, and Brier scores.

The attention mechanism is directly analogous to the attention-weighted
aggregation in:
  Piao et al., "GARNN: an interpretable graph attentive recurrent neural
  network …", Neural Networks 185, 107229, 2025.

The actor head outputs routing-weight logits (softmax → action).
The critic head outputs a scalar state-value for advantage estimation.
"""

from __future__ import annotations

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch import Tensor


class GATRouter(nn.Module):
    """Graph Attention routing policy + value network.

    Parameters
    ----------
    in_channels : int
        Node feature dimension (K + 3 from GraphBuilder).
    hidden_channels : int
        Hidden dim inside GAT layers.
    num_heads : int
        Number of attention heads per GAT layer.
    num_layers : int
        Number of GATv2Conv layers.
    num_makers : int
        N — fixed number of maker nodes (determines output width).
    dropout : float
        Dropout applied between GAT layers during training.
    """

    def __init__(
        self,
        in_channels: int,
        hidden_channels: int = 64,
        num_heads: int = 4,
        num_layers: int = 2,
        num_makers: int = 10,
        dropout: float = 0.1,
    ) -> None:
        super().__init__()
        from torch_geometric.nn import GATv2Conv

        self.num_makers = num_makers
        self.dropout = dropout

        self.convs = nn.ModuleList()
        self.norms = nn.ModuleList()

        for i in range(num_layers):
            in_dim = in_channels if i == 0 else hidden_channels * num_heads
            conv = GATv2Conv(
                in_channels=in_dim,
                out_channels=hidden_channels,
                heads=num_heads,
                dropout=dropout,
                add_self_loops=True,
            )
            self.convs.append(conv)
            self.norms.append(nn.LayerNorm(hidden_channels * num_heads))

        backbone_out = hidden_channels * num_heads

        self.actor_head = nn.Sequential(
            nn.Linear(backbone_out, hidden_channels),
            nn.ReLU(),
            nn.Linear(hidden_channels, 1),
        )

        self.critic_head = nn.Sequential(
            nn.Linear(backbone_out, hidden_channels),
            nn.ReLU(),
            nn.Linear(hidden_channels, 1),
        )

    def forward(
        self,
        x: Tensor,
        edge_index: Tensor,
    ) -> tuple[Tensor, Tensor]:
        """Forward pass.

        Parameters
        ----------
        x : Tensor (N, F)
        edge_index : Tensor (2, E)

        Returns
        -------
        action_logits : Tensor (N,)
            Un-normalised routing weights; apply softmax externally.
        state_value : Tensor (1,)
            Estimated value of the current state.
        """
        h = x
        for conv, norm in zip(self.convs, self.norms):
            h = conv(h, edge_index)
            h = norm(h)
            h = F.elu(h)
            h = F.dropout(h, p=self.dropout, training=self.training)

        action_logits = self.actor_head(h).squeeze(-1)
        node_values = self.critic_head(h).squeeze(-1)
        state_value = node_values.mean(dim=0, keepdim=True)

        return action_logits, state_value

    def act(
        self, x: Tensor, edge_index: Tensor, deterministic: bool = False
    ) -> tuple[Tensor, Tensor, Tensor]:
        """Sample an action and return (action, log_prob, value).

        Actions are routing weights obtained by sampling from a Dirichlet
        parameterised by softmax(logits).  In deterministic mode the
        softmax output is returned directly.
        """
        logits, value = self.forward(x, edge_index)
        probs = F.softmax(logits, dim=0)

        if deterministic:
            action = probs
            log_prob = torch.log(probs + 1e-8).sum()
        else:
            concentration = probs * self.num_makers + 1e-2
            dist = torch.distributions.Dirichlet(concentration)
            action = dist.sample()
            log_prob = dist.log_prob(action)

        return action, log_prob, value.squeeze()
