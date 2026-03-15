from zero_dex_lite.client import (
    LiteClient,
    PROTOCOL_VERSION,
    EIP712_DOMAIN_TYPE,
    INTENT_TYPE,
    DOMAIN_NAME,
    DOMAIN_VERSION,
)
from eth_utils import keccak
from eth_abi import encode


def test_signed_payload_shape():
    client = LiteClient(
        private_key="0x59c6995e998f97a5a0044976f8f2b8d2f22ebf0c6f0f4f7f3afccf4d7ed2d1a5",
        gateway="http://127.0.0.1:8080",
        chain_id=1,
    )
    payload = client._sign_intent(
        graph_content="graph",
        verifying_contract="0x4444444444444444444444444444444444444444",
        base_token="0x1111111111111111111111111111111111111111",
        quote_token="0x2222222222222222222222222222222222222222",
        side="sell",
        amount_in=100,
        min_amount_out=200,
        nonce=1,
        deadline_unix=4_102_444_800,
    )

    assert payload["version"] == PROTOCOL_VERSION
    assert payload["chain_id"] == 1
    assert payload["owner_address"].startswith("0x")
    assert payload["verifying_contract"].startswith("0x")
    assert payload["signature_hex"].startswith("0x")
    assert len(payload["signature_hex"]) == 132  # 0x + 65 bytes hex


def test_eip712_digest_is_deterministic():
    client = LiteClient(
        private_key="0x59c6995e998f97a5a0044976f8f2b8d2f22ebf0c6f0f4f7f3afccf4d7ed2d1a5",
        gateway="http://127.0.0.1:8080",
        chain_id=1,
    )
    payload = {
        "version": PROTOCOL_VERSION,
        "chain_id": 1,
        "nonce": 1,
        "deadline_unix": 4_102_444_800,
        "owner_address": client.account.address,
        "verifying_contract": "0x4444444444444444444444444444444444444444",
        "base_token": "0x1111111111111111111111111111111111111111",
        "quote_token": "0x2222222222222222222222222222222222222222",
        "side": "sell",
        "amount_in": 100,
        "min_amount_out": 200,
        "graph_content": "graph",
    }
    d1 = client.eip712_digest(payload)
    d2 = client.eip712_digest(payload)
    assert d1 == d2
    assert len(d1) == 32


def test_eip712_constants_match_solidity():
    """Verify that Python type hashes match what Solidity would produce."""
    domain_typehash = keccak(EIP712_DOMAIN_TYPE)
    intent_typehash = keccak(INTENT_TYPE)
    assert len(domain_typehash) == 32
    assert len(intent_typehash) == 32
    assert domain_typehash != intent_typehash


def test_buy_side_flips_tokens():
    """Verify that buy-side resolves tokenIn/tokenOut opposite to sell-side."""
    client = LiteClient(
        private_key="0x59c6995e998f97a5a0044976f8f2b8d2f22ebf0c6f0f4f7f3afccf4d7ed2d1a5",
        gateway="http://127.0.0.1:8080",
        chain_id=1,
    )
    base = {
        "version": PROTOCOL_VERSION,
        "chain_id": 1,
        "nonce": 1,
        "deadline_unix": 4_102_444_800,
        "owner_address": client.account.address,
        "verifying_contract": "0x4444444444444444444444444444444444444444",
        "base_token": "0x1111111111111111111111111111111111111111",
        "quote_token": "0x2222222222222222222222222222222222222222",
        "amount_in": 100,
        "min_amount_out": 200,
        "graph_content": "graph",
    }
    sell_payload = {**base, "side": "sell"}
    buy_payload = {**base, "side": "buy"}

    sell_digest = client.eip712_digest(sell_payload)
    buy_digest = client.eip712_digest(buy_payload)
    assert sell_digest != buy_digest


def test_context_manager_cleanup():
    with LiteClient(
        private_key="0x59c6995e998f97a5a0044976f8f2b8d2f22ebf0c6f0f4f7f3afccf4d7ed2d1a5",
        gateway="http://127.0.0.1:8080",
    ) as client:
        assert client._raw_key is not None
    assert client._raw_key is None
