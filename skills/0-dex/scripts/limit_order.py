import os
import sys
import json
import argparse

# In a real environment, the python package zero_dex_lite would be installed.
# For the skill execution, we assume it's available in the PYTHONPATH.
try:
    from zero_dex_lite import LiteClient, create_limit_order
except ImportError:
    print(json.dumps({"error": "zero_dex_lite SDK is not installed. Run 'pip install zero-dex-lite'."}))
    sys.exit(1)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--buy_asset', required=True)
    parser.add_argument('--sell_asset', required=True)
    parser.add_argument('--min_price', type=float, required=True)
    parser.add_argument('--amount', type=float, required=True)
    args = parser.parse_args()

    private_key = os.environ.get("ZERO_DEX_KEY")
    gateway = os.environ.get("ZERO_DEX_GATEWAY", "http://127.0.0.1:8080")

    if not private_key:
        print(json.dumps({"error": "ZERO_DEX_KEY environment variable is not set."}))
        sys.exit(1)

    try:
        # Initialize the zero-friction client
        client = LiteClient(private_key=private_key, gateway=gateway)
        
        # Generate the 0-lang mathematical graph
        graph = create_limit_order(
            buy_asset=args.buy_asset,
            sell_asset=args.sell_asset,
            min_price=args.min_price,
            amount=args.amount
        )
        
        # Sign and broadcast to the P2P mempool
        response = client.broadcast_intent(graph)
        
        # Output strictly structured JSON for the Agent to read
        print(json.dumps({
            "status": "success",
            "message": f"Broadcasted {args.buy_asset}/{args.sell_asset} intent to 0-dex network.",
            "details": response,
            "owner_address": client.account.address,
            "graph_snippet": graph.split('\n')[:5] # show first few lines for context
        }))
        
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
