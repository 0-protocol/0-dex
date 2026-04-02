"""Gymnasium RL environments for dark-pool routing research."""

from gymnasium.envs.registration import register

register(
    id="ZeroTVLDarkPool-v0",
    entry_point="envs.dark_pool_env:DarkPoolEnv",
)
