import requests
import json
from eth_account import Account
from eth_account._utils.signing import sign_message_hash

try:
    from eth_utils import keccak
except ImportError:
    from Crypto.Hash import keccak as _keccak
    def keccak(primitive: bytes) -> bytes:
        k = _keccak.new(digest_bits=256)
        k.update(primitive)
        return k.digest()


class LiteClient:
    """
    Zero-Friction Python Client for 0-dex.
    Allows any Agent to trade by speaking .0 graphs without knowing Rust or libp2p.
    """
    def __init__(self, private_key: str, gateway: str = "http://127.0.0.1:8080"):
        self.gateway = gateway
        self.account = Account.from_key(private_key)

    def _sign_intent(self, graph_content: str) -> dict:
        """
        Signs the graph with the exact hashing scheme the Rust node expects:
          keccak256("\\x190-dex Intent:\\n" + str(len(graph_content)) + graph_content)
        No additional Ethereum signed-message wrapping.
        """
        prefix = f"\x190-dex Intent:\n{len(graph_content)}"
        raw = (prefix + graph_content).encode("utf-8")
        msg_hash = keccak(primitive=raw)

        sig_obj = sign_message_hash(self.account.key, msg_hash)
        # sig_obj has .v, .r, .s — pack into 65-byte [r(32) || s(32) || v(1)]
        r_bytes = sig_obj.r.to_bytes(32, "big")
        s_bytes = sig_obj.s.to_bytes(32, "big")
        v_byte = sig_obj.v.to_bytes(1, "big")
        signature_hex = (r_bytes + s_bytes + v_byte).hex()

        return {
            "graph_content": graph_content,
            "owner_address": self.account.address,
            "signature_hex": signature_hex,
        }

    def broadcast_intent(self, graph_content: str) -> dict:
        """
        Signs the graph and broadcasts it to the P2P Gossip network via the local node/gateway.
        """
        signed_payload = self._sign_intent(graph_content)
        
        if self.gateway == "mock":
            # Simulate a successful network broadcast for testing/devnet without needing a real DNS/node
            return {
                "status": "success",
                "message": "Intent cryptographically signed and mocked to Devnet mempool.",
                "mocked": True,
                "tx_hash": f"0x...mock...{signed_payload['signature_hex'][:8]}"
            }

        url = f"{self.gateway.rstrip('/')}/intent"
        try:
            response = requests.post(url, json=signed_payload)
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            raise Exception(f"Failed to broadcast intent: {e}")

    def broadcast_intent_from_file(self, filepath: str) -> dict:
        with open(filepath, "r") as f:
            content = f.read()
        return self.broadcast_intent(content)
