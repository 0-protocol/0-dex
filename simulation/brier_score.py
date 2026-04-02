"""Brier-score penalty tracker for Byzantine-resilient routing.

The Brier score quantifies the squared discrepancy between a maker's
*reported* liquidity and the *realised* fill behaviour.  Accumulated over
time, it provides a reputation signal that the RL reward function uses to
penalise routing through unreliable nodes.

    BS_i = (1/T) * sum_t (reported_{i,t} - actual_{i,t})^2

The integration into the RL reward is:

    reward -= alpha * sum_i (w_i * BS_i)

where w_i is the routing weight assigned to maker i.  This drives the
policy to assign near-zero weight to nodes with high BS, effectively
isolating Byzantine makers.

Reference
---------
Dong et al., "Defending against poisoning attacks in federated learning
with blockchain", IEEE Transactions on Artificial Intelligence, 2024.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


class BrierScoreTracker:
    """Running per-node Brier scores.

    Parameters
    ----------
    num_nodes : int
        Number of federated maker nodes (N).
    window : int or None
        If set, only the last *window* observations contribute (sliding
        window).  None means the full history is used.
    """

    def __init__(self, num_nodes: int, window: int | None = None) -> None:
        self.num_nodes = num_nodes
        self.window = window
        self._history: list[NDArray[np.float64]] = []

    def reset(self) -> None:
        self._history.clear()

    def update(
        self,
        reported: NDArray[np.float32],
        actual: NDArray[np.float32],
    ) -> None:
        """Record one time-step of reported vs actual liquidity.

        Both arrays have shape (N,) — the total liquidity reported / realised
        per node (summed across price levels).
        """
        reported = np.asarray(reported, dtype=np.float64)
        actual = np.asarray(actual, dtype=np.float64)
        denom = max(float(actual.max()), 1e-9)
        sq_err = ((reported - actual) / denom) ** 2
        self._history.append(sq_err)

        if self.window is not None and len(self._history) > self.window:
            self._history = self._history[-self.window:]

    @property
    def scores(self) -> NDArray[np.float64]:
        """Current per-node Brier scores, shape (N,)."""
        if not self._history:
            return np.zeros(self.num_nodes, dtype=np.float64)
        return np.mean(self._history, axis=0)

    def penalty(self, weights: NDArray[np.float32]) -> float:
        """Compute weighted Brier penalty: sum_i w_i * BS_i."""
        return float(np.dot(np.asarray(weights, dtype=np.float64), self.scores))
