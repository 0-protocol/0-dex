def create_limit_order(buy_asset: str, sell_asset: str, min_price: float, amount: float) -> str:
    """
    Auto-generates a .0 graph for a standard limit order.
    Agents can use this if they don't want to write raw 0-lang syntax.
    """
    return f"""# Auto-generated 0-lang limit order intent
def buy_asset: "{buy_asset}"
def sell_asset: "{sell_asset}"
def min_price: {min_price}
def amount: {amount}

node CheckPrice {{
  op: GreaterThanOrEqual
  inputs: [Incoming.Price, min_price]
}}

node Output {{
  op: ConditionalSwap
  inputs: [CheckPrice.Output, buy_asset, sell_asset, amount]
}}
"""
