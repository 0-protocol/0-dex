"""Hash-based tensor commitment scheme.

Provides a Poseidon-style commitment (using SHA-256 as a stand-in when
``py_ecc`` is not available) for liquidity tensors.  The commitment is
binding: a maker cannot change its claimed balances after publishing the
commitment hash.

The verification statement is:

    H(balance_0 || balance_1 || ... || balance_{K-1} || salt) == commitment
    AND  sum(balances) >= min_liquidity_bound

This mirrors the ZK gradient-commitment scheme in the ZK-FL paper:
  Wang et al., IEEE TBD 2024 — Section IV-B.

The existing Rust stub (``src/privacy/zk.rs``) defines ``ZkEnvelope`` and
``ZkPublicOutputs``.  This Python module implements the *research-grade*
verification logic that the Rust TODO references.
"""

from __future__ import annotations

import hashlib
import os
import struct
from dataclasses import dataclass

import numpy as np
from numpy.typing import NDArray


@dataclass(frozen=True)
class TensorCommitment:
    """A commitment to a vector of balances."""

    commitment_hash: bytes
    salt: bytes
    min_liquidity_bound: float

    @classmethod
    def commit(
        cls,
        balances: NDArray[np.float64],
        min_liquidity_bound: float,
        salt: bytes | None = None,
    ) -> "TensorCommitment":
        """Create a binding commitment to *balances*.

        Parameters
        ----------
        balances : array (K,)
            Actual token balances per price level.
        min_liquidity_bound : float
            Public lower bound the prover wants to demonstrate.
        salt : bytes or None
            32-byte blinding factor.  Generated randomly if omitted.
        """
        if salt is None:
            salt = os.urandom(32)
        h = _hash_balances(balances, salt)
        return cls(commitment_hash=h, salt=salt, min_liquidity_bound=min_liquidity_bound)

    def verify(self, balances: NDArray[np.float64]) -> bool:
        """Verify that *balances* match the commitment and satisfy the bound.

        In a real ZK system the verifier never sees *balances* directly;
        here we expose the check for testing / simulation purposes.
        """
        h = _hash_balances(balances, self.salt)
        if h != self.commitment_hash:
            return False
        if float(balances.sum()) < self.min_liquidity_bound - 1e-9:
            return False
        return True


def _hash_balances(balances: NDArray[np.float64], salt: bytes) -> bytes:
    """SHA-256( b0 || b1 || ... || bK || salt ).

    In production this would be a Poseidon hash inside a SNARK-friendly
    field; SHA-256 is used here for portability.
    """
    h = hashlib.sha256()
    for b in balances.flat:
        h.update(struct.pack("<d", float(b)))
    h.update(salt)
    return h.digest()
