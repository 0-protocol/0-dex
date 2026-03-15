import os
import sys
import json
import argparse
import time

# In a real environment, the python package zero_dex_lite would be installed.
# For the skill execution, we assume it's available in the PYTHONPATH.
try:
    from zero_dex_lite import LiteClient
except ImportError:
    print(json.dumps({"error": "zero_dex_lite SDK is not installed. Run 'pip install zero-dex-lite'."}))
    sys.exit(1)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--base_token', required=True)
    parser.add_argument('--quote_token', required=True)
    parser.add_argument('--side', choices=["buy", "sell"], required=True)
    parser.add_argument('--amount_in', type=int, required=True)
    parser.add_argument('--min_amount_out', type=int, required=True)
    parser.add_argument('--nonce', type=int, default=int(time.time()))
    parser.add_argument('--deadline_unix', type=int, default=int(time.time()) + 600)
    parser.add_argument('--graph_content', default='{"strategy":"limit"}')
    args = parser.parse_args()

    private_key = os.environ.get("ZERO_DEX_KEY")
    gateway = os.environ.get("ZERO_DEX_GATEWAY", "http://127.0.0.1:8080")
    verifying_contract = os.environ.get("ZERO_DEX_ESCROW_ADDRESS")
    chain_id = int(os.environ.get("ZERO_DEX_CHAIN_ID", "1"))

    if not private_key or not verifying_contract:
        print(json.dumps({"error": "ZERO_DEX_KEY and ZERO_DEX_ESCROW_ADDRESS environment variables are required."}))
        sys.exit(1)

    try:
        client = LiteClient(
            private_key=private_key,
            gateway=gateway,
            chain_id=chain_id
        )
        response = client.broadcast_intent(
            graph_content=args.graph_content,
            verifying_contract=verifying_contract,
            base_token=args.base_token,
            quote_token=args.quote_token,
            side=args.side,
            amount_in=args.amount_in,
            min_amount_out=args.min_amount_out,
            nonce=args.nonce,
            deadline_unix=args.deadline_unix,
        )

        print(json.dumps({
            "status": "success",
            "message": f"Broadcasted {args.side} intent to 0-dex network.",
            "details": response,
            "owner_address": client.account.address,
            "base_token": args.base_token,
            "quote_token": args.quote_token,
            "amount_in": args.amount_in,
            "min_amount_out": args.min_amount_out,
        }))
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
