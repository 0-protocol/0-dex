"""Dynamic graph construction from dark-pool observations.

Converts a flat (N, K) observation tensor into a ``torch_geometric.data.Data``
object suitable for message-passing on a maker-node graph.

Node features per maker i:
    [reported_liquidity (K dims), historical_fill_rate, brier_score,
     time_since_last_update]

Edges connect all maker pairs that share at least one active price level
(fully connected by default, but prunable via *edge_threshold*).
"""

from __future__ import annotations

from typing import Optional

import numpy as np
import torch
from numpy.typing import NDArray


class GraphBuilder:
    """Builds PyG ``Data`` objects from environment observations.

    Parameters
    ----------
    num_makers : int
    num_levels : int
    edge_threshold : float
        Minimum observation magnitude to create an edge between two makers.
        Set to 0 for a fully connected graph.
    """

    def __init__(
        self,
        num_makers: int,
        num_levels: int,
        edge_threshold: float = 0.0,
    ) -> None:
        self.num_makers = num_makers
        self.num_levels = num_levels
        self.edge_threshold = edge_threshold

        self._fill_history: NDArray[np.float32] = np.zeros(num_makers, dtype=np.float32)
        self._brier_scores: NDArray[np.float32] = np.zeros(num_makers, dtype=np.float32)
        self._ages: NDArray[np.float32] = np.zeros(num_makers, dtype=np.float32)

    def reset(self) -> None:
        self._fill_history[:] = 0.0
        self._brier_scores[:] = 0.0
        self._ages[:] = 0.0

    def update_stats(
        self,
        fill_rates: NDArray[np.float32] | None = None,
        brier_scores: NDArray[np.float32] | None = None,
    ) -> None:
        """Incrementally update historical node features from step info."""
        if fill_rates is not None:
            alpha = 0.1
            self._fill_history = (
                (1 - alpha) * self._fill_history + alpha * np.asarray(fill_rates, dtype=np.float32)
            )
        if brier_scores is not None:
            self._brier_scores = np.asarray(brier_scores, dtype=np.float32)
        self._ages += 1.0

    def build(self, obs: NDArray[np.float32]) -> "torch_geometric.data.Data":  # noqa: F821
        """Convert an (N, K) observation to a PyG Data object.

        Returns
        -------
        torch_geometric.data.Data
            x:           (N, K+3) node feature matrix
            edge_index:  (2, E) COO edge tensor
        """
        from torch_geometric.data import Data

        obs = np.asarray(obs, dtype=np.float32)

        extra = np.stack(
            [self._fill_history, self._brier_scores, self._ages], axis=-1
        )
        x = np.concatenate([obs, extra], axis=-1)
        x_tensor = torch.from_numpy(x)

        edge_index = self._build_edges(obs)

        return Data(x=x_tensor, edge_index=edge_index)

    def _build_edges(self, obs: NDArray[np.float32]) -> torch.Tensor:
        """Fully-connected graph, optionally pruned by threshold."""
        src, dst = [], []
        for i in range(self.num_makers):
            for j in range(self.num_makers):
                if i == j:
                    continue
                if self.edge_threshold > 0:
                    mag = float(obs[i].sum() + obs[j].sum())
                    if mag < self.edge_threshold:
                        continue
                src.append(i)
                dst.append(j)
        return torch.tensor([src, dst], dtype=torch.long)

    @property
    def node_feature_dim(self) -> int:
        return self.num_levels + 3
