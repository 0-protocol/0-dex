"""ZeroTVLDarkPool-v0  —  Gymnasium environment for adversarial dark-pool routing.

The agent must split an order across N federated maker nodes using only
*ephemeral, decaying* liquidity reports as observations.  Some makers may be
Byzantine (spoofing high liquidity but refusing fills).

Observation
    (N, K) float32 tensor — reported liquidity at K price levels per maker,
    multiplied by exp(-lambda * age) temporal decay.

Action
    (N,) float32 — raw routing logits; softmax-normalised internally to
    produce routing weights that sum to 1.

Reward
    fill_rate * price_improvement - slippage_penalty - brier_penalty

    The Brier penalty directly implements the mechanism from:
      Dong et al., "Defending against poisoning attacks in federated learning
      with blockchain", IEEE TAI 2024.

References
----------
Wang et al., "Zero-Knowledge Proof-Based Gradient Aggregation for Federated
Learning", IEEE Transactions on Big Data, 2024.
"""

from __future__ import annotations

from typing import Any

import gymnasium as gym
import numpy as np
from gymnasium import spaces
from numpy.typing import NDArray

from envs.ephemeral_tensor import EphemeralTensorBuffer
from envs.orderbook import OrderBook


class DarkPoolEnv(gym.Env):
    """Dark-pool order routing as a continuous-control RL problem.

    Parameters (passed via ``kwargs`` or ``env_config``):
        num_makers      – N, number of federated maker nodes (default 10)
        num_levels      – K, price levels per maker (default 5)
        max_steps       – episode length (default 100)
        order_size      – notional order to fill each step (default 50.0)
        decay_lambda    – observation staleness rate (default 0.1)
        base_liquidity  – mean book depth per level (default 100.0)
        volatility      – book random-walk sigma (default 5.0)
        byzantine_mask  – boolean array (N,); True = Byzantine node
        brier_alpha     – Brier penalty weight in reward (default 0.5)
        seed            – random seed
    """

    metadata = {"render_modes": ["ansi"]}

    def __init__(self, **kwargs: Any) -> None:
        super().__init__()
        self.num_makers: int = kwargs.get("num_makers", 10)
        self.num_levels: int = kwargs.get("num_levels", 5)
        self.max_steps: int = kwargs.get("max_steps", 100)
        self.order_size: float = kwargs.get("order_size", 50.0)
        self.decay_lambda: float = kwargs.get("decay_lambda", 0.1)
        self.base_liquidity: float = kwargs.get("base_liquidity", 100.0)
        self.volatility: float = kwargs.get("volatility", 5.0)
        self.brier_alpha: float = kwargs.get("brier_alpha", 0.5)

        seed = kwargs.get("seed", None)
        self._rng = np.random.default_rng(seed)

        byz = kwargs.get("byzantine_mask", None)
        if byz is not None:
            self.byzantine_mask: NDArray[np.bool_] = np.asarray(byz, dtype=bool)
        else:
            self.byzantine_mask = np.zeros(self.num_makers, dtype=bool)

        self.observation_space = spaces.Box(
            low=0.0,
            high=np.inf,
            shape=(self.num_makers, self.num_levels),
            dtype=np.float32,
        )
        self.action_space = spaces.Box(
            low=-np.inf,
            high=np.inf,
            shape=(self.num_makers,),
            dtype=np.float32,
        )

        self._book = OrderBook(
            num_makers=self.num_makers,
            num_levels=self.num_levels,
            base_liquidity=self.base_liquidity,
            volatility=self.volatility,
            rng=self._rng,
        )
        self._obs_buf = EphemeralTensorBuffer(
            num_makers=self.num_makers,
            num_levels=self.num_levels,
            decay_lambda=self.decay_lambda,
            rng=self._rng,
        )

        self._step_count = 0
        self._cumulative_brier: NDArray[np.float64] = np.zeros(self.num_makers)
        self._brier_count = 0

    # ------------------------------------------------------------------
    # Gymnasium API
    # ------------------------------------------------------------------

    def reset(
        self,
        *,
        seed: int | None = None,
        options: dict[str, Any] | None = None,
    ) -> tuple[NDArray[np.float32], dict[str, Any]]:
        super().reset(seed=seed)
        if seed is not None:
            self._rng = np.random.default_rng(seed)

        self._book.reset()
        self._step_count = 0
        self._cumulative_brier = np.zeros(self.num_makers)
        self._brier_count = 0

        reports = self._generate_reports()
        self._obs_buf.reset(reports)

        obs = self._obs_buf.observe()
        info: dict[str, Any] = {"true_liquidity": self._book.true_liquidity()}
        return obs, info

    def step(
        self, action: NDArray[np.float32]
    ) -> tuple[NDArray[np.float32], float, bool, bool, dict[str, Any]]:
        action = np.asarray(action, dtype=np.float32).flatten()
        weights = _softmax(action)

        total_filled = 0.0
        requested_per_maker = weights * self.order_size

        fills = np.zeros(self.num_makers, dtype=np.float32)
        for i in range(self.num_makers):
            if self.byzantine_mask[i]:
                fills[i] = 0.0
            else:
                fills[i] = self._book.execute_fill(i, float(requested_per_maker[i]))
            total_filled += fills[i]

        fill_rate = total_filled / max(self.order_size, 1e-9)

        ideal_fill = min(
            self.order_size,
            float(self._book.true_liquidity()[~self.byzantine_mask].sum()),
        )
        price_improvement = total_filled / max(ideal_fill, 1e-9)

        slippage = float(np.sum(np.maximum(requested_per_maker - fills, 0.0)))
        slippage_penalty = slippage / max(self.order_size, 1e-9)

        reports = self._generate_reports()
        true_liq = self._book.true_liquidity()
        reported_total = reports.sum(axis=1)
        true_total = true_liq.sum(axis=1)
        max_liq = max(float(true_total.max()), 1e-9)
        brier_per_node = ((reported_total - true_total) / max_liq) ** 2
        self._cumulative_brier += brier_per_node
        self._brier_count += 1
        avg_brier = self._cumulative_brier / max(self._brier_count, 1)
        brier_penalty = float(np.dot(weights, avg_brier))

        reward = float(
            fill_rate * price_improvement
            - slippage_penalty
            - self.brier_alpha * brier_penalty
        )

        self._book.step()
        self._obs_buf.tick()
        for i in range(self.num_makers):
            self._obs_buf.update_report(i, reports[i])

        self._step_count += 1
        terminated = False
        truncated = self._step_count >= self.max_steps

        obs = self._obs_buf.observe()
        info: dict[str, Any] = {
            "fill_rate": fill_rate,
            "brier_scores": avg_brier.copy(),
            "weights": weights.copy(),
            "total_filled": total_filled,
        }
        return obs, reward, terminated, truncated, info

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _generate_reports(self) -> NDArray[np.float32]:
        """Each maker reports liquidity; Byzantine makers inflate theirs."""
        true_liq = self._book.true_liquidity()
        reports = true_liq.copy()
        for i in range(self.num_makers):
            if self.byzantine_mask[i]:
                inflate = self._rng.uniform(2.0, 5.0)
                reports[i] = true_liq[i] * inflate
        return reports


def _softmax(x: NDArray[np.float32]) -> NDArray[np.float32]:
    e = np.exp(x - x.max())
    return (e / e.sum()).astype(np.float32)
