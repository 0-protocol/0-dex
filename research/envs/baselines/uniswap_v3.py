"""Uniswap V3 concentrated-liquidity AMM baseline (Section 5).

Simulates a constant-product AMM with concentrated liquidity positions
to measure capital efficiency = volume_traded / TVL_locked.

FLARE claims >100x capital efficiency improvement over Uniswap V3
because its zero-TVL architecture requires no locked capital.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


class UniswapV3Baseline:
    """Constant-product AMM with concentrated liquidity simulation.

    Parameters
    ----------
    tvl : float
        Total Value Locked across all positions.
    fee_bps : int
        Swap fee in basis points (default 30 = 0.3%).
    num_ticks : int
        Number of concentrated liquidity ticks.
    rng : np.random.Generator
    """

    def __init__(
        self,
        tvl: float = 1_000_000.0,
        fee_bps: int = 30,
        num_ticks: int = 100,
        rng: np.random.Generator | None = None,
    ) -> None:
        self.tvl = tvl
        self.fee_rate = fee_bps / 10_000
        self.num_ticks = num_ticks
        self.rng = rng or np.random.default_rng()
        self._total_volume = 0.0

    def reset(self) -> None:
        self._total_volume = 0.0

    def execute_swap(self, order_size: float) -> dict[str, float]:
        """Execute a swap against the AMM pool.

        Returns dict with filled amount, slippage, and capital efficiency.
        """
        reserve = self.tvl / 2
        k = reserve * reserve

        amount_in = order_size * (1 - self.fee_rate)
        new_reserve_in = reserve + amount_in
        new_reserve_out = k / new_reserve_in
        amount_out = reserve - new_reserve_out

        price_impact = 1.0 - (amount_out / order_size) if order_size > 0 else 0.0

        self._total_volume += order_size

        return {
            "filled": float(amount_out),
            "slippage": float(max(price_impact, 0.0)),
            "tvl": self.tvl,
            "volume": self._total_volume,
            "capital_efficiency": self._total_volume / max(self.tvl, 1e-9),
        }

    @property
    def capital_efficiency(self) -> float:
        return self._total_volume / max(self.tvl, 1e-9)
