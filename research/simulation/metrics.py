"""Paper-specific evaluation metrics (Section 5).

Three core metrics promised in the FLARE experimental design:

1. Capital Efficiency  -- volume_traded / TVL_locked
2. Effective Slippage  -- volume-weighted price impact
3. Byzantine Resilience -- ability to isolate adversarial nodes
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


def capital_efficiency(volume_traded: float, tvl_locked: float) -> float:
    """Capital efficiency ratio = volume / TVL.

    For FLARE (zero-TVL), TVL is 0 by design, yielding infinite efficiency.
    For AMM baselines (Uniswap V3), this measures volume per unit locked.
    Returns float('inf') when tvl_locked <= 0.
    """
    if tvl_locked <= 0:
        return float("inf")
    return volume_traded / tvl_locked


def effective_slippage(
    intended_amount: float,
    executed_amount: float,
) -> float:
    """Effective slippage = 1 - executed / intended.

    A value of 0 means no slippage (perfect fill).
    A value of 1 means complete failure to fill.
    """
    if intended_amount <= 0:
        return 0.0
    return 1.0 - executed_amount / intended_amount


def byzantine_resilience(
    weights: NDArray[np.float32],
    byzantine_mask: NDArray[np.bool_],
    threshold: float = 0.01,
) -> float:
    """Byzantine resilience score.

    Returns the fraction of Byzantine nodes whose routing weight has
    been driven below *threshold*.  A score of 1.0 means the agent has
    fully isolated all Byzantine nodes.

    Parameters
    ----------
    weights : (N,) array
        Current routing weights assigned by the agent.
    byzantine_mask : (N,) bool array
        True for Byzantine nodes.
    threshold : float
        Weight below which a node is considered "isolated".
    """
    byz_weights = weights[byzantine_mask]
    if len(byz_weights) == 0:
        return 1.0
    isolated = (byz_weights < threshold).sum()
    return float(isolated) / len(byz_weights)


def convergence_episode(
    weight_history: list[NDArray[np.float32]],
    byzantine_mask: NDArray[np.bool_],
    threshold: float = 0.01,
) -> int | None:
    """Find the first episode where all Byzantine weights drop below threshold.

    Returns None if convergence was never achieved.
    Used to validate Theorem 4.1 (eta sweep experiments).
    """
    for ep, weights in enumerate(weight_history):
        if byzantine_resilience(weights, byzantine_mask, threshold) >= 1.0:
            return ep
    return None
