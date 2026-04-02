"""Byzantine-resilient dark-pool simulation framework.

Extends the FLock Byzantine-fault-tolerant federated learning framework
(Dong et al., IEEE TAI 2024) to DeFi order routing: malicious makers
submit spoofed liquidity tensors while the RL agent learns to isolate
them via a Brier-score reward penalty.
"""

from simulation.federation import HonestMaker, Federation
from simulation.byzantine import SpooferMaker, SandwichMaker, FreeRiderMaker
from simulation.brier_score import BrierScoreTracker

__all__ = [
    "HonestMaker",
    "Federation",
    "SpooferMaker",
    "SandwichMaker",
    "FreeRiderMaker",
    "BrierScoreTracker",
]
