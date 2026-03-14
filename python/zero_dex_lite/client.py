import requests
import json
from eth_account import Account
from eth_account.messages import encode_defunct

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
        Cryptographically ties the 0-lang graph to the Agent's wallet.
        Matches the \x190-dex Intent:\n length prefix hashing required by the Rust node.
        """
        prefix = f"\x190-dex Intent:\n{len(graph_content)}"
        payload = prefix + graph_content
        
        # We use encode_defunct to hash the raw string
        message = encode_defunct(text=payload)
        signed_message = self.account.sign_message(message)
        
        return {
            "graph_content": graph_content,
            "owner_address": self.account.address,
            "signature_hex": signed_message.signature.hex()
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
