import ctypes
import requests
from typing import Optional
from eth_abi import encode
from eth_account import Account
from eth_account.messages import SignableMessage
from eth_utils import keccak, to_checksum_address

PROTOCOL_VERSION = "0-dex-v1"
EIP712_DOMAIN_TYPE = b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
INTENT_TYPE = b"Intent(address owner,address tokenIn,address tokenOut,uint256 amountIn,uint256 minAmountOut,uint256 nonce,uint256 deadline)"
DOMAIN_NAME = b"ZeroDexEscrow"
DOMAIN_VERSION = b"1"


def _try_zero_bytes(b: bytes) -> None:
    try:
        buf = (ctypes.c_char * len(b)).from_address(id(b) + bytes.__basicsize__)
        ctypes.memset(buf, 0, len(b))
    except Exception:
        pass


class LiteClient:
    """
    Zero-Friction Python Client for 0-dex.
    """

    def __init__(self, private_key: str, gateway: str = "http://127.0.0.1:8080", chain_id: int = 1):
        self.gateway = gateway
        self.chain_id = chain_id
        self._raw_key: Optional[bytes] = bytes.fromhex(private_key.replace("0x", ""))
        self.account = Account.from_key(private_key)

    def close(self) -> None:
        if self._raw_key is not None:
            _try_zero_bytes(self._raw_key)
            self._raw_key = None

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def _sign_intent(
        self,
        graph_content: str,
        verifying_contract: str,
        base_token: str,
        quote_token: str,
        side: str,
        amount_in: int,
        min_amount_out: int,
        nonce: int,
        deadline_unix: int,
    ) -> dict:
        payload = {
            "version": PROTOCOL_VERSION,
            "chain_id": self.chain_id,
            "nonce": nonce,
            "deadline_unix": deadline_unix,
            "owner_address": self.account.address,
            "verifying_contract": verifying_contract,
            "base_token": base_token,
            "quote_token": quote_token,
            "side": side.lower(),
            "amount_in": int(amount_in),
            "min_amount_out": int(min_amount_out),
            "graph_content": graph_content,
        }
        domain_separator, struct_hash = self._eip712_components(payload)
        signable = SignableMessage(version=b"", header=domain_separator, body=struct_hash)
        signed_message = self.account.sign_message(signable)

        signed_payload = dict(payload)
        signed_payload["signature_hex"] = "0x" + signed_message.signature.hex()
        return signed_payload

    def broadcast_intent(
        self,
        graph_content: str,
        verifying_contract: str,
        base_token: str,
        quote_token: str,
        side: str,
        amount_in: int,
        min_amount_out: int,
        nonce: int,
        deadline_unix: int,
        api_key: Optional[str] = None,
    ) -> dict:
        signed_payload = self._sign_intent(
            graph_content=graph_content,
            verifying_contract=verifying_contract,
            base_token=base_token,
            quote_token=quote_token,
            side=side,
            amount_in=amount_in,
            min_amount_out=min_amount_out,
            nonce=nonce,
            deadline_unix=deadline_unix,
        )

        url = f"{self.gateway.rstrip('/')}/intent"
        headers = {}
        if api_key:
            headers["x-zero-dex-api-key"] = api_key
        response = requests.post(url, json=signed_payload, headers=headers, timeout=10)
        response.raise_for_status()
        return response.json()

    def broadcast_intent_from_file(
        self,
        filepath: str,
        verifying_contract: str,
        base_token: str,
        quote_token: str,
        side: str,
        amount_in: int,
        min_amount_out: int,
        nonce: int,
        deadline_unix: int,
        api_key: Optional[str] = None,
    ) -> dict:
        with open(filepath, "r") as f:
            content = f.read()
        return self.broadcast_intent(
            graph_content=content,
            verifying_contract=verifying_contract,
            base_token=base_token,
            quote_token=quote_token,
            side=side,
            amount_in=amount_in,
            min_amount_out=min_amount_out,
            nonce=nonce,
            deadline_unix=deadline_unix,
            api_key=api_key,
        )

    def _eip712_components(self, payload: dict) -> tuple:
        token_in, token_out = self._resolved_tokens(payload["side"], payload["base_token"], payload["quote_token"])

        domain_separator = keccak(
            encode(
                ["bytes32", "bytes32", "bytes32", "uint256", "address"],
                [
                    keccak(EIP712_DOMAIN_TYPE),
                    keccak(DOMAIN_NAME),
                    keccak(DOMAIN_VERSION),
                    int(payload["chain_id"]),
                    to_checksum_address(payload["verifying_contract"]),
                ],
            )
        )

        struct_hash = keccak(
            encode(
                ["bytes32", "address", "address", "address", "uint256", "uint256", "uint256", "uint256"],
                [
                    keccak(INTENT_TYPE),
                    to_checksum_address(payload["owner_address"]),
                    to_checksum_address(token_in),
                    to_checksum_address(token_out),
                    int(payload["amount_in"]),
                    int(payload["min_amount_out"]),
                    int(payload["nonce"]),
                    int(payload["deadline_unix"]),
                ],
            )
        )
        return domain_separator, struct_hash

    def eip712_digest(self, payload: dict) -> bytes:
        domain_separator, struct_hash = self._eip712_components(payload)
        return keccak(b"" + domain_separator + struct_hash)

    @staticmethod
    def _resolved_tokens(side: str, base_token: str, quote_token: str):
        normalized = side.lower()
        if normalized == "sell":
            return base_token, quote_token
        if normalized == "buy":
            return quote_token, base_token
        raise ValueError(f"Unsupported side: {side}")
