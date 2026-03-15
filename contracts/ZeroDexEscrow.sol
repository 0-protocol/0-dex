// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity ^0.8.20;

interface IERC20 {
    function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

contract ZeroDexEscrow {
    struct Intent {
        address owner;
        address tokenIn;
        address tokenOut;
        uint256 amountIn;
        uint256 minAmountOut;
        uint256 nonce;
        uint256 deadline;
        uint256 chainId;
    }

    bytes32 public constant EIP712_DOMAIN_TYPEHASH =
        keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");

    bytes32 public constant INTENT_TYPEHASH =
        keccak256("Intent(address owner,address tokenIn,address tokenOut,uint256 amountIn,uint256 minAmountOut,uint256 nonce,uint256 deadline)");

    bytes32 private constant NAME_HASH = keccak256("ZeroDexEscrow");
    bytes32 private constant VERSION_HASH = keccak256("1");

    mapping(address => mapping(uint256 => bool)) public nonceUsed;
    mapping(bytes32 => bool) public matchExecuted;
    uint256 private _lock = 1;

    error InvalidIntent();
    error InvalidSignature();
    error NonceAlreadyUsed();
    error IntentExpired();
    error ChainIdMismatch();
    error MatchAlreadyExecuted();
    error ReentrancyBlocked();
    error TransferFailed();
    error InvalidTokenContract();
    error BalanceDeltaMismatch();

    event TradeSettled(
        bytes32 indexed matchId,
        address indexed partyA,
        address indexed partyB,
        address tokenA,
        address tokenB,
        uint256 amountA,
        uint256 amountB
    );

    modifier nonReentrant() {
        if (_lock != 1) revert ReentrancyBlocked();
        _lock = 2;
        _;
        _lock = 1;
    }

    function executeSwap(
        Intent calldata intentA,
        Intent calldata intentB,
        uint256 amountA,
        uint256 amountB,
        bytes32 matchId,
        bytes calldata signatureA,
        bytes calldata signatureB
    ) external nonReentrant {
        _validateIntent(intentA);
        _validateIntent(intentB);

        if (intentA.owner == intentB.owner) revert InvalidIntent();
        if (intentA.tokenIn != intentB.tokenOut || intentA.tokenOut != intentB.tokenIn) revert InvalidIntent();
        if (amountA > intentA.amountIn || amountB > intentB.amountIn) revert InvalidIntent();
        if (amountB < intentA.minAmountOut || amountA < intentB.minAmountOut) revert InvalidIntent();
        if (matchExecuted[matchId]) revert MatchAlreadyExecuted();

        bytes32 digestA = _intentDigest(intentA);
        bytes32 digestB = _intentDigest(intentB);
        _verifySignature(intentA.owner, digestA, signatureA);
        _verifySignature(intentB.owner, digestB, signatureB);

        nonceUsed[intentA.owner][intentA.nonce] = true;
        nonceUsed[intentB.owner][intentB.nonce] = true;
        matchExecuted[matchId] = true;

        _safeTransferChecked(intentA.tokenIn, intentA.owner, intentB.owner, amountA);
        _safeTransferChecked(intentB.tokenIn, intentB.owner, intentA.owner, amountB);

        emit TradeSettled(matchId, intentA.owner, intentB.owner, intentA.tokenIn, intentB.tokenIn, amountA, amountB);
    }

    function domainSeparator() public view returns (bytes32) {
        return keccak256(abi.encode(
            EIP712_DOMAIN_TYPEHASH,
            NAME_HASH,
            VERSION_HASH,
            block.chainid,
            address(this)
        ));
    }

    function _validateIntent(Intent calldata intent) internal view {
        if (intent.owner == address(0) || intent.tokenIn == address(0) || intent.tokenOut == address(0)) {
            revert InvalidIntent();
        }
        if (intent.tokenIn.code.length == 0 || intent.tokenOut.code.length == 0) {
            revert InvalidTokenContract();
        }
        if (intent.deadline < block.timestamp) revert IntentExpired();
        if (intent.chainId != block.chainid) revert ChainIdMismatch();
        if (nonceUsed[intent.owner][intent.nonce]) revert NonceAlreadyUsed();
    }

    function _intentDigest(Intent calldata intent) internal view returns (bytes32) {
        bytes32 structHash = keccak256(abi.encode(
            INTENT_TYPEHASH,
            intent.owner,
            intent.tokenIn,
            intent.tokenOut,
            intent.amountIn,
            intent.minAmountOut,
            intent.nonce,
            intent.deadline
        ));
        return keccak256(abi.encodePacked("\x19\x01", domainSeparator(), structHash));
    }

    function _verifySignature(address expectedSigner, bytes32 digest, bytes calldata signature) internal pure {
        if (signature.length != 65) revert InvalidSignature();
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := calldataload(signature.offset)
            s := calldataload(add(signature.offset, 32))
            v := byte(0, calldataload(add(signature.offset, 64)))
        }
        if (v < 27) v += 27;
        if (v != 27 && v != 28) revert InvalidSignature();
        address recovered = ecrecover(digest, v, r, s);
        if (recovered == address(0) || recovered != expectedSigner) revert InvalidSignature();
    }

    function _safeTransferFrom(
        address token,
        address from,
        address to,
        uint256 amount
    ) internal {
        (bool success, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.transferFrom.selector, from, to, amount)
        );
        if (!success) revert TransferFailed();
        if (data.length > 0 && !abi.decode(data, (bool))) revert TransferFailed();
    }

    function _safeTransferChecked(address token, address from, address to, uint256 amount) internal {
        uint256 fromBefore = _balanceOf(token, from);
        uint256 toBefore = _balanceOf(token, to);
        if (fromBefore < amount) revert BalanceDeltaMismatch();

        _safeTransferFrom(token, from, to, amount);

        uint256 fromAfter = _balanceOf(token, from);
        uint256 toAfter = _balanceOf(token, to);
        if (fromAfter != fromBefore - amount || toAfter != toBefore + amount) {
            revert BalanceDeltaMismatch();
        }
    }

    function _balanceOf(address token, address owner) internal view returns (uint256) {
        (bool ok, bytes memory data) = token.staticcall(
            abi.encodeWithSelector(IERC20.balanceOf.selector, owner)
        );
        if (!ok || data.length < 32) revert InvalidTokenContract();
        return abi.decode(data, (uint256));
    }
}
