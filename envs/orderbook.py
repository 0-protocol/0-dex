"""Hidden order-book simulation backing the dark-pool environment.

The order book is the *ground truth* that the RL agent never sees directly.
Each federated maker node owns a slice of the book; honest makers fill
orders proportionally to their true depth while Byzantine makers may lie.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


class OrderBook:
    """Per-maker hidden order book with K price levels.

    Parameters
    ----------
    num_makers : int
        Number of federated maker nodes (N).
    num_levels : int
        Number of discrete price levels per maker (K).
    base_liquidity : float
        Mean liquidity seeded at each level on reset.
    volatility : float
        Std-dev of the Gaussian noise applied to liquidity each step.
    rng : np.random.Generator
        Controlled random state for reproducibility.
    """

    def __init__(
        self,
        num_makers: int,
        num_levels: int,
        base_liquidity: float = 100.0,
        volatility: float = 5.0,
        rng: np.random.Generator | None = None,
    ) -> None:
        self.num_makers = num_makers
        self.num_levels = num_levels
        self.base_liquidity = base_liquidity
        self.volatility = volatility
        self.rng = rng or np.random.default_rng()
        self.books: NDArray[np.float32] = np.zeros(
            (num_makers, num_levels), dtype=np.float32
        )

    def reset(self) -> None:
        self.books = (
            self.rng.normal(
                self.base_liquidity,
                self.volatility,
                size=(self.num_makers, self.num_levels),
            )
            .clip(min=0.0)
            .astype(np.float32)
        )

    def step(self) -> None:
        """Evolve liquidity by one time-step (random walk)."""
        delta = self.rng.normal(
            0.0,
            self.volatility,
            size=(self.num_makers, self.num_levels),
        ).astype(np.float32)
        self.books = (self.books + delta).clip(min=0.0)

    def true_liquidity(self) -> NDArray[np.float32]:
        """Return the full hidden book — never exposed to the agent."""
        return self.books.copy()

    def execute_fill(
        self,
        maker_idx: int,
        requested_amount: float,
    ) -> float:
        """Attempt to fill *requested_amount* against maker's true depth.

        Returns the actually filled amount (capped by available liquidity).
        Liquidity is consumed from the lowest price level upward.
        """
        filled = 0.0
        for k in range(self.num_levels):
            available = float(self.books[maker_idx, k])
            take = min(available, requested_amount - filled)
            self.books[maker_idx, k] -= take
            filled += take
            if filled >= requested_amount - 1e-9:
                break
        return filled
