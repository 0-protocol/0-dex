"""1inch-style deterministic aggregator baseline (Section 5).

Routes orders greedily to the maker with the highest reported depth.
No learning, no probabilistic reasoning, no temporal decay.

FLARE claims superior effective slippage over deterministic aggregators
because it accounts for information staleness and adversarial reports.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


class OneInchAggregatorBaseline:
    """Greedy deterministic order-splitting aggregator.

    Splits orders proportionally to reported liquidity depth, without
    any discount for staleness or adversarial behaviour.
    """

    def compute_weights(self, reported_liquidity: NDArray[np.float32]) -> NDArray[np.float32]:
        """Compute routing weights from raw reported liquidity.

        Parameters
        ----------
        reported_liquidity : (N, K) array
            Reported depth per maker per level.

        Returns
        -------
        weights : (N,) array summing to 1.
        """
        totals = reported_liquidity.sum(axis=1)
        denom = totals.sum()
        if denom < 1e-9:
            return np.ones(len(totals), dtype=np.float32) / len(totals)
        return (totals / denom).astype(np.float32)

    def execute(
        self,
        order_size: float,
        reported_liquidity: NDArray[np.float32],
        true_liquidity: NDArray[np.float32],
        byzantine_mask: NDArray[np.bool_],
    ) -> dict[str, float]:
        """Execute a deterministic greedy split.

        Returns dict with filled, slippage, and weights.
        """
        weights = self.compute_weights(reported_liquidity)
        requested = weights * order_size

        fills = np.zeros(len(weights), dtype=np.float32)
        for i in range(len(weights)):
            if byzantine_mask[i]:
                fills[i] = 0.0
            else:
                avail = float(true_liquidity[i].sum())
                fills[i] = min(float(requested[i]), avail)

        total_filled = float(fills.sum())
        slippage = 1.0 - total_filled / max(order_size, 1e-9)

        return {
            "filled": total_filled,
            "slippage": float(max(slippage, 0.0)),
            "weights": weights,
        }
