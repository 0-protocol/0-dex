"""Brier-score penalty tracker for Byzantine-resilient routing.

Implements the classic Brier score as defined in the FLARE paper (below
Eq. 2):

    B_i^{(t)} = (T_{i,p} - I_{fill}^{(i)})^2

where T_{i,p} is the maker's declared fill probability (from the
Ephemeral Tensor) and I_{fill} in {0, 1} is the binary fill indicator.
The cumulative score is the running mean over time:

    BS_i = (1/T) * sum_t B_i^{(t)}

Integration into the RL reward (Eq. 2):

    R = sum_i w_i * (alpha * Gain_i - eta * BS_i)

This drives the policy to assign near-zero weight to nodes with high BS,
effectively isolating Byzantine makers.

Reference
---------
Dong et al., "Defending against poisoning attacks in federated learning
with blockchain", IEEE Transactions on Artificial Intelligence, 2024.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


class BrierScoreTracker:
    """Running per-node Brier scores using (predicted_prob - indicator)^2.

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
        predicted_fill_prob: NDArray[np.float32],
        actual_fill: NDArray[np.bool_],
    ) -> None:
        """Record one time-step of predicted probability vs binary outcome.

        Parameters
        ----------
        predicted_fill_prob : (N,) float array
            Each maker's declared fill probability T_{i,p}, clipped to [0,1].
        actual_fill : (N,) bool array
            Whether each maker actually filled (True) or refused (False).
        """
        pred = np.clip(np.asarray(predicted_fill_prob, dtype=np.float64), 0.0, 1.0)
        indicator = np.asarray(actual_fill, dtype=np.float64)
        brier = (pred - indicator) ** 2
        self._history.append(brier)

        if self.window is not None and len(self._history) > self.window:
            self._history = self._history[-self.window:]

    @property
    def scores(self) -> NDArray[np.float64]:
        """Current per-node Brier scores BS_i, shape (N,)."""
        if not self._history:
            return np.zeros(self.num_nodes, dtype=np.float64)
        return np.mean(self._history, axis=0)

    def penalty(self, weights: NDArray[np.float32]) -> float:
        """Compute weighted Brier penalty: sum_i w_i * BS_i."""
        return float(np.dot(np.asarray(weights, dtype=np.float64), self.scores))
