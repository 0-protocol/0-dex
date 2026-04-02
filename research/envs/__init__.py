"""Gymnasium RL environments for dark-pool routing research.

Reviewer entry points
---------------------
* :mod:`research.envs.darkpool_pomdp` — POMDP formulation (Section 3)
* :mod:`research.envs.dark_pool_env`  — full implementation
"""

from gymnasium.envs.registration import register

register(
    id="ZeroTVLDarkPool-v0",
    entry_point="research.envs.dark_pool_env:DarkPoolEnv",
)
