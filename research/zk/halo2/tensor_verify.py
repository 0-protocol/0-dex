"""Halo2-style tensor commitment verification — Python pseudocode.

This module sketches how the same ZK statement proved by
``zk/circom/tensor_proof.circom`` would be expressed as a Halo2 circuit
using the IPA (Inner Product Argument) commitment scheme.

It is *not* executable Halo2 code (which is Rust); instead it serves as
annotated pseudocode for the paper appendix, making the algorithm
accessible to ML researchers unfamiliar with Rust circuit DSLs.

The Halo2 approach is attractive for 0-dex because:
  - No trusted setup (unlike Groth16 used by Circom + snarkjs).
  - Recursive composition allows proving batches of tensor commitments.
  - Aligns with the recursive proof aggregation discussed in:
      Wang et al., "ZK-FL", IEEE TBD 2024, Section V.

Pseudocode structure mirrors the Halo2 API:
  1. ``TensorCommitmentConfig``  — column layout
  2. ``TensorCommitmentCircuit`` — witness assignment + constraints
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


# --------------------------------------------------------------------------
# Mock Halo2 primitives (for readability; not functional)
# --------------------------------------------------------------------------

class Field:
    """Represents an element of the BN254 / Pasta scalar field."""
    def __init__(self, value: int = 0):
        self.value = value


class Column:
    """A column in the Halo2 PLONKish table."""
    def __init__(self, name: str, kind: str = "advice"):
        self.name = name
        self.kind = kind  # "advice", "instance", "fixed"


class Region:
    """Assignment region inside a Halo2 layouter."""
    def assign_advice(self, column: Column, row: int, value: Field) -> "Cell":
        return Cell(column, row, value)

    def constrain_equal(self, a: "Cell", b: "Cell") -> None:
        pass


@dataclass
class Cell:
    column: Column
    row: int
    value: Field


# --------------------------------------------------------------------------
# Circuit definition
# --------------------------------------------------------------------------

@dataclass
class TensorCommitmentConfig:
    """Column layout for the tensor commitment circuit.

    Columns
    -------
    balance : advice
        One row per price level, holding the private balance value.
    running_sum : advice
        Accumulates sum(balances) row by row.
    salt : advice
        Single-row blinding factor.
    hash_out : advice
        Poseidon sponge state / output.
    commitment : instance (public)
        The published commitment hash.
    min_bound : instance (public)
        The minimum liquidity threshold.
    """
    balance: Column = None       # type: ignore[assignment]
    running_sum: Column = None   # type: ignore[assignment]
    salt: Column = None          # type: ignore[assignment]
    hash_out: Column = None      # type: ignore[assignment]
    commitment: Column = None    # type: ignore[assignment]
    min_bound: Column = None     # type: ignore[assignment]

    @classmethod
    def configure(cls) -> "TensorCommitmentConfig":
        return cls(
            balance=Column("balance", "advice"),
            running_sum=Column("running_sum", "advice"),
            salt=Column("salt", "advice"),
            hash_out=Column("hash_out", "advice"),
            commitment=Column("commitment", "instance"),
            min_bound=Column("min_bound", "instance"),
        )


class TensorCommitmentCircuit:
    """Halo2 circuit proving a liquidity tensor matches a public commitment.

    Pseudocode for the paper appendix.

    Parameters
    ----------
    balances : list[int]
        Private balance values (K elements).
    salt : int
        Blinding factor.
    commitment : int
        Public Poseidon hash.
    min_liquidity : int
        Public lower bound.
    """

    def __init__(
        self,
        balances: list[int],
        salt: int,
        commitment: int,
        min_liquidity: int,
    ):
        self.balances = balances
        self.salt = salt
        self.commitment = commitment
        self.min_liquidity = min_liquidity

    def configure(self) -> TensorCommitmentConfig:
        """Set up the constraint system (gates + columns)."""
        return TensorCommitmentConfig.configure()

    def synthesize(self, config: TensorCommitmentConfig) -> None:
        """Assign witnesses and enforce constraints.

        This is the core of the Halo2 circuit.  In real Halo2 Rust code,
        ``synthesize`` is called by the prover with a ``Layouter``.
        """
        region = Region()

        # --- 1. Assign private balances ---
        balance_cells: list[Cell] = []
        for i, b in enumerate(self.balances):
            cell = region.assign_advice(config.balance, row=i, value=Field(b))
            balance_cells.append(cell)

        # --- 2. Compute running sum and constrain ---
        acc = 0
        for i, b in enumerate(self.balances):
            acc += b
            region.assign_advice(config.running_sum, row=i, value=Field(acc))

        total_sum = acc

        # --- 3. Assign salt ---
        salt_cell = region.assign_advice(config.salt, row=0, value=Field(self.salt))

        # --- 4. Poseidon hash gate ---
        #   hash_out = Poseidon(balances[0], ..., balances[K-1], salt)
        #
        #   In real Halo2: use a PoseidonChip with rate=K+1.
        #   The chip adds its own advice columns and custom gates.
        #   Here we just note the constraint:
        hash_cell = region.assign_advice(
            config.hash_out, row=0, value=Field(self.commitment)
        )

        # --- 5. Constrain hash_out == public commitment ---
        commitment_cell = Cell(config.commitment, 0, Field(self.commitment))
        region.constrain_equal(hash_cell, commitment_cell)

        # --- 6. Range check: total_sum >= min_liquidity ---
        #   In Halo2: decompose (total_sum - min_liquidity) into bits
        #   and constrain all bits to be 0 or 1 (ensuring non-negative).
        diff = total_sum - self.min_liquidity
        assert diff >= 0, "Witness does not satisfy min_liquidity bound"

        # --- Done ---
        #   The verifier checks:
        #     (a) Poseidon(balances, salt) == commitment  (binding)
        #     (b) sum(balances) >= min_liquidity           (solvency)
        #   without learning the individual balance values.
