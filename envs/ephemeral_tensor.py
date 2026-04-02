"""Ephemeral probability tensors with temporal decay.

Each maker node periodically reports a liquidity tensor.  The observation
the RL agent receives is *not* the raw report but a decayed version:

    obs_{i,k} = reported_{i,k} * exp(-lambda * delta_t_i)

where delta_t_i is the number of steps since maker i last reported.
This models information staleness inherent in dark-pool networks.

Reference
---------
The decay mechanism mirrors the temporal attention weighting in:
  Piao et al., "GARNN: an interpretable graph attentive recurrent neural
  network for predicting blood glucose levels via multivariate time series",
  Neural Networks 185, 107229, 2025.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


class EphemeralTensorBuffer:
    """Maintains decayed observation tensors for N makers x K price levels.

    Parameters
    ----------
    num_makers : int
    num_levels : int
    decay_lambda : float
        Exponential decay rate.  Higher = faster staleness.
    rng : np.random.Generator
    """

    def __init__(
        self,
        num_makers: int,
        num_levels: int,
        decay_lambda: float = 0.1,
        rng: np.random.Generator | None = None,
    ) -> None:
        self.num_makers = num_makers
        self.num_levels = num_levels
        self.decay_lambda = decay_lambda
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

    def observe(self) -> NDArray[np.float32]:
        """Return the decayed observation tensor (N, K)."""
        decay = np.exp(-self.decay_lambda * self._ages).astype(np.float32)
        return self._raw_reports * decay[:, np.newaxis]
