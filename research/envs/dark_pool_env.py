"""ZeroTVLDarkPool-v0  --  POMDP Gymnasium environment for adversarial dark-pool routing.

Formalises the zero-TVL federated dark-pool routing problem as a Partially
Observable Markov Decision Process (POMDP) and wraps it as a standard
Gymnasium environment.

POMDP Tuple  <S, A, O, P, R, gamma>  (Section 3 of the FLARE paper)
------------------------------------------------------------------------
S -- System hidden state s_t = {LOB_i(t)}_{i=1}^N.
     The true Limit Order Book of every federated maker.  Stored internally
     in ``OrderBook`` and **strictly invisible** to the agent.

A -- Action a_t = [w_1, ..., w_N], sum(w_i) = 1.
     Continuous routing weight vector over N maker nodes.

O -- Observation o_t: Ephemeral Tensors (Definition 3.1).
     T_{i,p}(v, t) = P(Fill_i=1 | Vol=v) * exp(-lambda_i * (t - t_0))

P -- Transition: hidden order books evolve via a Gaussian random walk
     each step; fills consume liquidity.

R -- Reward (Eq. 2):
     R(s, a) = sum_i w_i * (alpha * Gain_i - eta * B_i)
     where Gain_i is the per-maker fill gain and B_i is the Brier score.

gamma -- Discount factor (configured externally by the RL algorithm).

References
----------
Wang et al., "Zero-Knowledge Proof-Based Gradient Aggregation for Federated
Learning", IEEE Transactions on Big Data, 2024.

Dong et al., "Defending against poisoning attacks in federated learning
with blockchain", IEEE TAI 2024.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import gymnasium as gym
import numpy as np
from gymnasium import spaces
from numpy.typing import NDArray

from research.envs.ephemeral_tensor import EphemeralTensorBuffer
from research.envs.orderbook import OrderBook
from research.simulation.brier_score import BrierScoreTracker


# ------------------------------------------------------------------
# Formal POMDP mapping (Section 3)
# ------------------------------------------------------------------

@dataclass
class FlarePOMDP:
    r"""Formal mapping of the FLARE POMDP tuple <S, A, O, P, R, gamma>.

    Attributes correspond one-to-one to Section 3 of the paper.
    This dataclass is purely documentary; it is not instantiated at
    runtime but serves as a searchable reference for reviewers.

    S : set
        Hidden state space.  s_t = {LOB_i(t)} for i in 1..N.
        Each LOB_i is a (K,) vector of liquidity at K price levels.
    A : set
        Action space.  a_t = [w_1, ..., w_N] with sum(w_i) = 1.
        Continuous routing weights over N federated maker nodes.
    O : set
        Observation space.  Ephemeral Tensors (Definition 3.1):
        T_{i,p}(v, t) = P(Fill_i=1 | Vol=v) * exp(-lambda_i * dt).
    P : callable
        Transition kernel.  LOBs evolve via Gaussian random walk;
        fills consume liquidity from the hidden books.
    R : callable
        Reward function (Eq. 2):
        R(s, a) = sum_i w_i * (alpha * Gain_i(w_i) - eta * B_i^{(t)}).
    gamma : float
        Discount factor (set by the RL algorithm, not the environment).
    """

    S: str = "{LOB_i(t)}_{i=1}^N"
    A: str = "[w_1, ..., w_N], sum=1"
    O: str = "T_{i,p}(v,t) = P(Fill|Vol) * exp(-lambda*dt)"
    P: str = "Gaussian random walk + fill consumption"
    R: str = "sum_i w_i * (alpha * Gain_i - eta * B_i)"
    gamma: str = "configured by RL algorithm"


# ------------------------------------------------------------------
# Environment
# ------------------------------------------------------------------

class DarkPoolEnv(gym.Env):
    """Dark-pool order routing as a continuous-control POMDP.

    Implements the full FLARE formulation including:
      - Ephemeral Tensor observations with P(Fill|Vol) (Def. 3.1)
      - Per-maker reward decomposition (Eq. 2)
      - Brier score B_i = (T_{i,p} - I_fill)^2 via BrierScoreTracker

    Parameters (passed via ``kwargs``):
        num_makers      -- N, number of federated maker nodes (default 10)
        num_levels      -- K, price levels per maker (default 5)
        max_steps       -- episode length (default 100)
        order_size      -- notional order to fill each step (default 50.0)
        decay_lambda    -- observation staleness rate (default 0.1)
        base_liquidity  -- mean book depth per level (default 100.0)
        volatility      -- book random-walk sigma (default 5.0)
        byzantine_mask  -- boolean array (N,); True = Byzantine node
        alpha           -- gain coefficient in Eq. 2 (default 1.0)
        eta             -- Brier penalty coefficient in Eq. 2 (default 0.5)
        gamma_env       -- not used (gamma is external to the env)
        seed            -- random seed
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
        self.alpha: float = kwargs.get("alpha", 1.0)
        self.eta: float = kwargs.get("eta", 0.5)

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
        self._brier = BrierScoreTracker(self.num_makers)

        self._step_count = 0

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
        self._brier.reset()
        self._step_count = 0

        reports = self._generate_reports()
        self._obs_buf.reset(reports)

        obs = self._obs_buf.observe(requested_volume=self.order_size)
        info: dict[str, Any] = {"true_liquidity": self._book.true_liquidity()}
        return obs, info

    def step(
        self, action: NDArray[np.float32]
    ) -> tuple[NDArray[np.float32], float, bool, bool, dict[str, Any]]:
        action = np.asarray(action, dtype=np.float32).flatten()
        weights = _softmax(action)

        requested_per_maker = weights * self.order_size

        # --- Execute fills ---
        fills = np.zeros(self.num_makers, dtype=np.float32)
        for i in range(self.num_makers):
            if self.byzantine_mask[i]:
                fills[i] = 0.0
            else:
                fills[i] = self._book.execute_fill(i, float(requested_per_maker[i]))

        # --- Brier score: B_i = (T_{i,p} - I_fill)^2  (below Eq. 2) ---
        fill_prob = self._obs_buf.fill_probability(self.order_size)
        actual_filled = fills > 1e-9
        self._brier.update(fill_prob, actual_filled)

        # --- Per-maker Gain_i (Eq. 2) ---
        gain_per_maker = np.zeros(self.num_makers, dtype=np.float64)
        for i in range(self.num_makers):
            if requested_per_maker[i] > 1e-9:
                gain_per_maker[i] = fills[i] / requested_per_maker[i]

        # --- Reward: R(s,a) = sum_i w_i * (alpha * Gain_i - eta * B_i) ---
        brier_scores = self._brier.scores
        per_maker_reward = self.alpha * gain_per_maker - self.eta * brier_scores
        reward = float(np.dot(weights, per_maker_reward))

        # --- Evolve hidden state & update observations ---
        reports = self._generate_reports()
        self._book.step()
        self._obs_buf.tick()
        for i in range(self.num_makers):
            self._obs_buf.update_report(i, reports[i])

        self._step_count += 1
        terminated = False
        truncated = self._step_count >= self.max_steps

        obs = self._obs_buf.observe(requested_volume=self.order_size)

        total_filled = float(fills.sum())
        fill_rate = total_filled / max(self.order_size, 1e-9)
        info: dict[str, Any] = {
            "fill_rate": fill_rate,
            "total_filled": total_filled,
            "brier_scores": brier_scores.copy(),
            "weights": weights.copy(),
            "fills": fills.copy(),
            "gain_per_maker": gain_per_maker.copy(),
            "fill_prob": fill_prob.copy(),
            "actual_filled": actual_filled.copy(),
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
