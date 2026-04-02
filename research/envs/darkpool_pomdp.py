"""POMDP environment for zero-TVL dark-pool routing (Section 3).

Reviewer-facing entry point.  The full implementation lives in
:mod:`research.envs.dark_pool_env` and includes:

* **POMDP tuple** ``<S, A, O, P, R, gamma>`` formalised as :class:`FlarePOMDP`
* **Ephemeral Tensor observations** (Definition 3.1):
  ``T_{i,p}(v,t) = P(Fill_i=1 | Vol=v) * exp(-lambda_i * (t - t_0))``
* **Brier-penalised reward** (Eq. 2):
  ``R(s,a) = sum_i w_i * (alpha * Gain_i - eta * B_i)``
  where ``B_i = (T_{i,p} - I_fill)^2``

Quick start::

    import gymnasium as gym
    import research.envs  # registers ZeroTVLDarkPool-v0

    env = gym.make(
        "ZeroTVLDarkPool-v0",
        num_makers=10,
        byzantine_mask=[False]*7 + [True]*3,
        alpha=1.0,
        eta=0.5,
    )
    obs, info = env.reset(seed=42)
    action = env.action_space.sample()
    obs, reward, terminated, truncated, info = env.step(action)
"""

from research.envs.dark_pool_env import DarkPoolEnv, FlarePOMDP

__all__ = ["DarkPoolEnv", "FlarePOMDP"]
