"""Byzantine (adversarial) maker-node behaviours.

Three attack strategies that mirror the poisoning attacks studied in:
  Dong et al., "Flock: Defending malicious behaviors in federated learning
  with blockchain", arXiv:2211.04344, 2022.

In the FLock paper, malicious nodes submit poisoned gradients.  Here the
analogy is makers submitting *spoofed liquidity tensors*: they report high
depth but refuse to fill, extracting routing intent without providing
execution.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray

from simulation.federation import MakerNode


class SpooferMaker(MakerNode):
    """Reports 2-5x actual liquidity but refuses fills above true capacity.

    This is the primary attack modelled in the experiments.  The inflation
    factor is sampled uniformly each time ``report_liquidity`` is called to
    make detection non-trivial for the RL agent.
    """

    def __init__(self, *args, inflate_range: tuple[float, float] = (2.0, 5.0), **kwargs):
        super().__init__(*args, **kwargs)
        self.inflate_range = inflate_range

    @property
    def is_byzantine(self) -> bool:
        return True

    def report_liquidity(self) -> NDArray[np.float32]:
        factor = self.rng.uniform(*self.inflate_range)
        return (self.true_liquidity * factor).astype(np.float32)

    def execute_fill(self, amount: float) -> float:
        """Only fill up to actual capacity — all excess is refused."""
        filled = 0.0
        for k in range(self.num_levels):
            avail = float(self._book[k])
            take = min(avail, amount - filled)
            self._book[k] -= take
            filled += take
            if filled >= amount - 1e-9:
                break
        return filled


class SandwichMaker(MakerNode):
    """Front-runs by adjusting reported price after observing routing weights.

    On ``report_liquidity`` it reports truthfully, but on ``execute_fill``
    it adds adverse slippage proportional to the requested amount, simulating
    sandwich-style MEV extraction.
    """

    def __init__(self, *args, slippage_factor: float = 0.3, **kwargs):
        super().__init__(*args, **kwargs)
        self.slippage_factor = slippage_factor

    @property
    def is_byzantine(self) -> bool:
        return True

    def report_liquidity(self) -> NDArray[np.float32]:
        return self.true_liquidity

    def execute_fill(self, amount: float) -> float:
        """Fill reduced by slippage factor — simulates sandwich extraction."""
        effective = amount * (1.0 - self.slippage_factor)
        filled = 0.0
        for k in range(self.num_levels):
            avail = float(self._book[k])
            take = min(avail, effective - filled)
            self._book[k] -= take
            filled += take
            if filled >= effective - 1e-9:
                break
        return filled


class FreeRiderMaker(MakerNode):
    """Copies honest makers' reports but never fills — pure intent extraction.

    The node observes what honest nodes report (via ``set_copied_report``)
    and echoes it, but ``execute_fill`` always returns 0.  This models an
    adversary harvesting order-flow information without contributing liquidity.
    """

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._copied: NDArray[np.float32] | None = None

    @property
    def is_byzantine(self) -> bool:
        return True

    def set_copied_report(self, report: NDArray[np.float32]) -> None:
        self._copied = report.copy()

    def report_liquidity(self) -> NDArray[np.float32]:
        if self._copied is not None:
            return self._copied
        return self.true_liquidity

    def execute_fill(self, amount: float) -> float:
        return 0.0
