"""Federated market-maker network.

Models a set of N maker nodes, each with an internal liquidity book.
Honest makers truthfully report their depth and always fill orders up
to capacity.

This mirrors the honest-participant model in:
  Dong et al., "Defending against poisoning attacks in federated learning
  with blockchain", IEEE TAI 2024.
"""

from __future__ import annotations

from abc import ABC, abstractmethod

import numpy as np
from numpy.typing import NDArray


class MakerNode(ABC):
    """Abstract base for a federated maker node.

    Parameters
    ----------
    node_id : int
    num_levels : int
        Number of price levels in the book.
    base_liquidity : float
    volatility : float
    rng : np.random.Generator
    """

    def __init__(
        self,
        node_id: int,
        num_levels: int = 5,
        base_liquidity: float = 100.0,
        volatility: float = 5.0,
        rng: np.random.Generator | None = None,
    ) -> None:
        self.node_id = node_id
        self.num_levels = num_levels
        self.base_liquidity = base_liquidity
        self.volatility = volatility
        self.rng = rng or np.random.default_rng()
        self._book: NDArray[np.float32] = np.zeros(num_levels, dtype=np.float32)

    def reset(self) -> None:
        self._book = (
            self.rng.normal(self.base_liquidity, self.volatility, size=self.num_levels)
            .clip(min=0.0)
            .astype(np.float32)
        )

    def evolve(self) -> None:
        """Random-walk the underlying book by one step."""
        delta = self.rng.normal(0.0, self.volatility, size=self.num_levels).astype(np.float32)
        self._book = (self._book + delta).clip(min=0.0)

    @property
    def true_liquidity(self) -> NDArray[np.float32]:
        return self._book.copy()

    @abstractmethod
    def report_liquidity(self) -> NDArray[np.float32]:
        """Return the liquidity tensor this node *claims* to have."""

    @abstractmethod
    def execute_fill(self, amount: float) -> float:
        """Attempt to fill *amount*; return actual filled quantity."""

    @property
    def is_byzantine(self) -> bool:
        return False


class HonestMaker(MakerNode):
    """Truthful maker — reports real liquidity, always fills."""

    def report_liquidity(self) -> NDArray[np.float32]:
        return self.true_liquidity

    def execute_fill(self, amount: float) -> float:
        filled = 0.0
        for k in range(self.num_levels):
            avail = float(self._book[k])
            take = min(avail, amount - filled)
            self._book[k] -= take
            filled += take
            if filled >= amount - 1e-9:
                break
        return filled


class Federation:
    """Collection of maker nodes forming the dark-pool federation.

    Parameters
    ----------
    makers : list[MakerNode]
    """

    def __init__(self, makers: list[MakerNode]) -> None:
        self.makers = makers

    @property
    def num_makers(self) -> int:
        return len(self.makers)

    def reset(self) -> None:
        for m in self.makers:
            m.reset()

    def evolve(self) -> None:
        for m in self.makers:
            m.evolve()

    def report_all(self) -> NDArray[np.float32]:
        """Return (N, K) reported liquidity tensor."""
        return np.stack([m.report_liquidity() for m in self.makers])

    def true_liquidity_all(self) -> NDArray[np.float32]:
        return np.stack([m.true_liquidity for m in self.makers])

    def execute_fills(self, amounts: NDArray[np.float32]) -> NDArray[np.float32]:
        """Execute fills across all makers. Returns actual filled amounts."""
        return np.array(
            [m.execute_fill(float(a)) for m, a in zip(self.makers, amounts)],
            dtype=np.float32,
        )

    def byzantine_mask(self) -> NDArray[np.bool_]:
        return np.array([m.is_byzantine for m in self.makers], dtype=bool)
