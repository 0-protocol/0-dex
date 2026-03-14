// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity ^0.8.24;

interface IERC20 {
    function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
}

/**
 * @title ZeroDexEscrow
 * @dev The settlement layer for the 0-dex Agent-Native P2P network.
 * 
 * Agents match intents off-chain using 0-lang Tensor intersections. Once a 
 * match is found, the SettlementEngine bundles their signatures and submits 
 * them here to execute the atomic swap.
 */
contract ZeroDexEscrow {
    
    bytes32 public DOMAIN_SEPARATOR;
    bytes32 public constant INTENT_TYPEHASH = keccak256(
        "Intent(address giveToken,address receiveToken,uint256 giveAmount,uint256 receiveAmount,uint256 nonce)"
    );

    mapping(address => uint256) public nonces;
    
    // Reentrancy guard
    uint256 private _status;
    uint256 private constant _NOT_ENTERED = 1;
    uint256 private constant _ENTERED = 2;

    event TradeSettled(
        address indexed partyA, 
        address indexed partyB, 
        address tokenA, 
        address tokenB, 
        uint256 amountA, 
        uint256 amountB
    );

    constructor() {
        uint256 chainId;
        assembly {
            chainId := chainid()
        }
        DOMAIN_SEPARATOR = keccak256(
            abi.encode(
                keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
                keccak256(bytes("0-dex Escrow")),
                keccak256(bytes("1")),
                chainId,
                address(this)
            )
        );
        _status = _NOT_ENTERED;
    }

    modifier nonReentrant() {
        require(_status != _ENTERED, "ReentrancyGuard: reentrant call");
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }

    /**
     * @notice Executes an atomic swap between two Agents whose 0-lang graphs intersected.
     * @param partyA The address of the first Agent.
     * @param partyB The address of the second Agent.
     * @param tokenA The address of the ERC20 token partyA is giving.
     * @param tokenB The address of the ERC20 token partyB is giving.
     * @param amountA The amount of tokenA.
     * @param amountB The amount of tokenB.
     * @param signatureA The cryptographic signature of partyA's intent.
     * @param signatureB The cryptographic signature of partyB's intent.
     */
    function executeSwap(
        address partyA,
        address partyB,
        address tokenA,
        address tokenB,
        uint256 amountA,
        uint256 amountB,
        bytes calldata signatureA,
        bytes calldata signatureB
    ) external nonReentrant {
        // 1. Verify EIP-712 signatures for both parties
        _verifySignature(partyA, tokenA, tokenB, amountA, amountB, nonces[partyA]++, signatureA);
        _verifySignature(partyB, tokenB, tokenA, amountB, amountA, nonces[partyB]++, signatureB);
        
        // 2. Perform the atomic swap
        require(IERC20(tokenA).transferFrom(partyA, partyB, amountA), "TokenA transfer failed");
        require(IERC20(tokenB).transferFrom(partyB, partyA, amountB), "TokenB transfer failed");
        
        emit TradeSettled(partyA, partyB, tokenA, tokenB, amountA, amountB);
    }

    function _verifySignature(
        address signer,
        address giveToken,
        address receiveToken,
        uint256 giveAmount,
        uint256 receiveAmount,
        uint256 nonce,
        bytes calldata signature
    ) internal view {
        require(signature.length == 65, "Invalid signature length");

        bytes32 structHash = keccak256(
            abi.encode(
                INTENT_TYPEHASH,
                giveToken,
                receiveToken,
                giveAmount,
                receiveAmount,
                nonce
            )
        );

        bytes32 digest = keccak256(
            abi.encodePacked(
                "\x19\x01",
                DOMAIN_SEPARATOR,
                structHash
            )
        );

        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := calldataload(signature.offset)
            s := calldataload(add(signature.offset, 32))
            v := byte(0, calldataload(add(signature.offset, 64)))
        }

        address recoveredSigner = ecrecover(digest, v, r, s);
        require(recoveredSigner != address(0) && recoveredSigner == signer, "Invalid signature");
    }
}
