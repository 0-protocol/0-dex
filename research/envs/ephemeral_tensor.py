"""Ephemeral probability tensors with temporal decay (Definition 3.1).

Implements the Ephemeral Tensor from the FLARE paper:

    T_{i,p}(v, t) = P(Fill_i = 1 | Vol = v) * exp(-lambda_i * (t - t_0))

where:
  - P(Fill_i = 1 | Vol = v) is the volume-conditioned fill probability,
    modelled as a logistic function of (reported_depth - requested_volume).
  - lambda_i is the per-node confidence decay factor.
  - (t - t_0) is the time elapsed since the maker's last report.

The fill probability captures the intuition that a maker reporting depth D
is more likely to fill a small order v << D than a large order v ~ D.
Byzantine nodes that inflate reports will have high P(Fill) but low actual
fill rates, which the Brier score (Eq. 2) penalises.

Reference
---------
Piao et al., "GARNN: an interpretable graph attentive recurrent neural
network for predicting blood glucose levels via multivariate time series",
Neural Networks 185, 107229, 2025.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


def _sigmoid(x: NDArray[np.float32]) -> NDArray[np.float32]:
    return (1.0 / (1.0 + np.exp(-np.clip(x, -20, 20)))).astype(np.float32)


class EphemeralTensorBuffer:
    """Maintains decayed observation tensors for N makers x K price levels.

    Observation at time t for maker i at level k:

        obs_{i,k} = P(Fill_i | Vol=v, level=k) * exp(-lambda * age_i)

    Parameters
    ----------
    num_makers : int
    num_levels : int
    decay_lambda : float
        Exponential decay rate.  Higher = faster staleness.
    fill_logistic_k : float
        Steepness of the logistic fill-probability curve.  Controls how
        sharply P(Fill) drops as requested volume approaches reported depth.
    rng : np.random.Generator
    """

    def __init__(
        self,
        num_makers: int,
        num_levels: int,
        decay_lambda: float = 0.1,
        fill_logistic_k: float = 0.05,
        rng: np.random.Generator | None = None,
    ) -> None:
        self.num_makers = num_makers
        self.num_levels = num_levels
        self.decay_lambda = decay_lambda
        self.fill_logistic_k = fill_logistic_k
        self.rng = rng or np.random.default_rng()

        self._raw_reports: NDArray[np.float32] = np.zeros(
            (num_makers, num_levels), dtype=np.float32
        )
        self._ages: NDArray[np.float32] = np.zeros(num_makers, dtype=np.float32)

    def reset(self, initial_reports: NDArray[np.float32]) -> None:
        self._raw_reports = initial_reports.copy()
        self._ages = np.zeros(self.num_makers, dtype=np.float32)

    def update_report(self, maker_idx: int, report: NDArray[np.float32]) -> None:
        """Record a fresh report from *maker_idx*, resetting its age."""
        self._raw_reports[maker_idx] = report
        self._ages[maker_idx] = 0.0

    def tick(self) -> None:
        """Advance all report ages by one time-step."""
        self._ages += 1.0

    def fill_probability(
        self, requested_volume: float
    ) -> NDArray[np.float32]:
        """P(Fill_i = 1 | Vol = v) per maker, shape (N,).

        Uses a logistic model: sigmoid(k * (total_reported_depth_i - v)).
        When reported depth >> v the probability is near 1; when v exceeds
        reported depth the probability drops toward 0.
        """
        total_reported = self._raw_reports.sum(axis=1)
        return _sigmoid(self.fill_logistic_k * (total_reported - requested_volume))

    def observe(
        self, requested_volume: float | None = None
    ) -> NDArray[np.float32]:
        """Return the Ephemeral Tensor T_{i,p}(v, t), shape (N, K).

        Implements Definition 3.1:
            T_{i,p}(v, t) = P(Fill_i=1 | Vol=v) * exp(-lambda * (t - t0))

        If *requested_volume* is None, P(Fill) is set to 1 (backward-compat).
        """
        decay = np.exp(-self.decay_lambda * self._ages).astype(np.float32)

        if requested_volume is not None:
            pfill = self.fill_probability(requested_volume)
            scale = pfill * decay
        else:
            scale = decay

        return self._raw_reports * scale[:, np.newaxis]
