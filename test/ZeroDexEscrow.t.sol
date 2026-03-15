// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../contracts/ZeroDexEscrow.sol";

contract MockERC20 {
    string public name;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    constructor(string memory _name) { name = _name; }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        require(balanceOf[from] >= amount, "insufficient");
        require(allowance[from][msg.sender] >= amount, "no allowance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        allowance[from][msg.sender] -= amount;
        return true;
    }
}

contract MockNoReturnERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external { balanceOf[to] += amount; }
    function approve(address spender, uint256 amount) external { allowance[msg.sender][spender] = amount; }

    function transferFrom(address from, address to, uint256 amount) external {
        require(balanceOf[from] >= amount, "insufficient");
        require(allowance[from][msg.sender] >= amount, "no allowance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        allowance[from][msg.sender] -= amount;
    }
}

contract ReentrantToken {
    ZeroDexEscrow public escrow;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    bool public attacking;

    bytes public attackPayload;

    constructor(address _escrow) { escrow = ZeroDexEscrow(_escrow); }

    function mint(address to, uint256 amount) external { balanceOf[to] += amount; }
    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function setAttackPayload(bytes calldata data) external { attackPayload = data; attacking = true; }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        require(balanceOf[from] >= amount, "insufficient");
        require(allowance[from][msg.sender] >= amount, "no allowance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        allowance[from][msg.sender] -= amount;
        if (attacking) {
            attacking = false;
            (bool ok, ) = address(escrow).call(attackPayload);
            // Expect the call to revert due to reentrancy guard — ok should be false
            require(!ok, "reentrancy should have been blocked");
        }
        return true;
    }
}

contract ZeroDexEscrowTest is Test {
    event TradeSettled(
        bytes32 indexed matchId,
        address indexed partyA,
        address indexed partyB,
        address tokenA,
        address tokenB,
        uint256 amountA,
        uint256 amountB
    );

    ZeroDexEscrow escrow;
    MockERC20 tokenA;
    MockERC20 tokenB;

    uint256 constant ALICE_PK = 0xA11CE;
    uint256 constant BOB_PK = 0xB0B;
    address alice;
    address bob;

    function setUp() public {
        escrow = new ZeroDexEscrow();
        tokenA = new MockERC20("TokenA");
        tokenB = new MockERC20("TokenB");
        alice = vm.addr(ALICE_PK);
        bob = vm.addr(BOB_PK);

        tokenA.mint(alice, 1000 ether);
        tokenB.mint(bob, 5000 ether);
        vm.prank(alice);
        tokenA.approve(address(escrow), type(uint256).max);
        vm.prank(bob);
        tokenB.approve(address(escrow), type(uint256).max);
    }

    // ───────────────── Helpers ─────────────────

    function _defaultIntentA() internal view returns (ZeroDexEscrow.Intent memory) {
        return ZeroDexEscrow.Intent({
            owner: alice,
            tokenIn: address(tokenA),
            tokenOut: address(tokenB),
            amountIn: 100 ether,
            minAmountOut: 300 ether,
            nonce: 1,
            deadline: block.timestamp + 1 hours,
            chainId: block.chainid
        });
    }

    function _defaultIntentB() internal view returns (ZeroDexEscrow.Intent memory) {
        return ZeroDexEscrow.Intent({
            owner: bob,
            tokenIn: address(tokenB),
            tokenOut: address(tokenA),
            amountIn: 400 ether,
            minAmountOut: 80 ether,
            nonce: 1,
            deadline: block.timestamp + 1 hours,
            chainId: block.chainid
        });
    }

    function _intentDigest(ZeroDexEscrow.Intent memory intent) internal view returns (bytes32) {
        bytes32 structHash = keccak256(abi.encode(
            escrow.INTENT_TYPEHASH(),
            intent.owner,
            intent.tokenIn,
            intent.tokenOut,
            intent.amountIn,
            intent.minAmountOut,
            intent.nonce,
            intent.deadline
        ));
        return keccak256(abi.encodePacked("\x19\x01", escrow.domainSeparator(), structHash));
    }

    function _sign(uint256 pk, bytes32 digest) internal pure returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(pk, digest);
        return abi.encodePacked(r, s, v);
    }

    function _executeDefault(uint256 amountA, uint256 amountB) internal {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));
        escrow.executeSwap(intentA, intentB, amountA, amountB, matchId, sigA, sigB);
    }

    // ───────────────── Happy Path ─────────────────

    function testSuccessfulSwap() public {
        uint256 aliceABefore = tokenA.balanceOf(alice);
        uint256 aliceBBefore = tokenB.balanceOf(alice);
        uint256 bobABefore = tokenA.balanceOf(bob);
        uint256 bobBBefore = tokenB.balanceOf(bob);

        _executeDefault(100 ether, 350 ether);

        assertEq(tokenA.balanceOf(alice), aliceABefore - 100 ether);
        assertEq(tokenB.balanceOf(alice), aliceBBefore + 350 ether);
        assertEq(tokenA.balanceOf(bob), bobABefore + 100 ether);
        assertEq(tokenB.balanceOf(bob), bobBBefore - 350 ether);
    }

    function testEmitsTradeSettledEvent() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectEmit(true, true, true, true);
        emit TradeSettled(matchId, alice, bob, address(tokenA), address(tokenB), 100 ether, 350 ether);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    function testNoncesMarkedUsedAfterSwap() public {
        assertFalse(escrow.nonceUsed(alice, 1));
        assertFalse(escrow.nonceUsed(bob, 1));
        _executeDefault(100 ether, 350 ether);
        assertTrue(escrow.nonceUsed(alice, 1));
        assertTrue(escrow.nonceUsed(bob, 1));
    }

    function testMatchIdMarkedExecuted() public {
        bytes32 matchId = keccak256("match1");
        assertFalse(escrow.matchExecuted(matchId));
        _executeDefault(100 ether, 350 ether);
        assertTrue(escrow.matchExecuted(matchId));
    }

    // ───────────────── Signature Verification ─────────────────

    function testRevertsOnInvalidSignatureA() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory badSigA = _sign(BOB_PK, _intentDigest(intentA)); // wrong signer
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.InvalidSignature.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, badSigA, sigB);
    }

    function testRevertsOnInvalidSignatureB() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory badSigB = _sign(ALICE_PK, _intentDigest(intentB)); // wrong signer

        vm.expectRevert(ZeroDexEscrow.InvalidSignature.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, badSigB);
    }

    function testRevertsOnShortSignature() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.InvalidSignature.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, hex"deadbeef", sigB);
    }

    function testRevertsOnTamperedIntentAmount() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        intentA.amountIn = 200 ether; // tamper after signing
        bytes32 matchId = keccak256("match1");
        vm.expectRevert(ZeroDexEscrow.InvalidSignature.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    // ───────────────── Replay Protection ─────────────────

    function testRevertsOnReplayedMatchId() public {
        _executeDefault(100 ether, 350 ether);

        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        intentA.nonce = 2;
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.nonce = 2;
        bytes32 matchId = keccak256("match1"); // same match ID
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.MatchAlreadyExecuted.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    function testRevertsOnReplayedNonce() public {
        _executeDefault(100 ether, 350 ether);

        ZeroDexEscrow.Intent memory intentA = _defaultIntentA(); // nonce 1 reused
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.nonce = 2;
        bytes32 matchId = keccak256("match2");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.NonceAlreadyUsed.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    // ───────────────── Expiry ─────────────────

    function testRevertsOnExpiredIntentA() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        intentA.deadline = block.timestamp - 1;
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.IntentExpired.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    function testRevertsOnExpiredIntentB() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.deadline = block.timestamp - 1;
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.IntentExpired.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    // ───────────────── Chain ID ─────────────────

    function testRevertsOnWrongChainId() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        intentA.chainId = 999;
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.ChainIdMismatch.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    // ───────────────── Token Pair Validation ─────────────────

    function testRevertsOnMismatchedTokenPair() public {
        MockERC20 tokenC = new MockERC20("TokenC");
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.tokenOut = address(tokenC); // A sends tokenA, B expects tokenC back — mismatch
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.InvalidIntent.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    function testRevertsOnSameOwner() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.owner = alice; // both same owner
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(ALICE_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.InvalidIntent.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    function testRevertsOnZeroOwner() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        intentA.owner = address(0);
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");

        vm.expectRevert(ZeroDexEscrow.InvalidIntent.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, hex"", hex"");
    }

    // ───────────────── Amount Bound Validation ─────────────────

    function testRevertsWhenAmountExceedsIntentA() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.InvalidIntent.selector);
        escrow.executeSwap(intentA, intentB, 200 ether, 350 ether, matchId, sigA, sigB);
    }

    function testRevertsWhenAmountBelowMinOut() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        // amountB (200e) < intentA.minAmountOut (300e)
        vm.expectRevert(ZeroDexEscrow.InvalidIntent.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 200 ether, matchId, sigA, sigB);
    }

    // ───────────────── Reentrancy ─────────────────

    function testReentrancyBlocked() public {
        ReentrantToken reentrant = new ReentrantToken(address(escrow));
        reentrant.mint(alice, 1000 ether);
        vm.prank(alice);
        reentrant.approve(address(escrow), type(uint256).max);

        ZeroDexEscrow.Intent memory intentA = ZeroDexEscrow.Intent({
            owner: alice,
            tokenIn: address(reentrant),
            tokenOut: address(tokenB),
            amountIn: 100 ether,
            minAmountOut: 300 ether,
            nonce: 1,
            deadline: block.timestamp + 1 hours,
            chainId: block.chainid
        });
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.tokenOut = address(reentrant);
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        // Set up the reentrant attack payload — the callback will try to call executeSwap again
        bytes memory reentryPayload = abi.encodeCall(
            escrow.executeSwap,
            (intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB)
        );
        reentrant.setAttackPayload(reentryPayload);

        // The outer call should succeed; the reentrancy attempt inside should fail silently
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    // ───────────────── Non-Standard ERC20 ─────────────────

    function testWorksWithNoReturnToken() public {
        MockNoReturnERC20 noReturnToken = new MockNoReturnERC20();
        noReturnToken.mint(alice, 1000 ether);
        vm.prank(alice);
        noReturnToken.approve(address(escrow), type(uint256).max);

        ZeroDexEscrow.Intent memory intentA = ZeroDexEscrow.Intent({
            owner: alice,
            tokenIn: address(noReturnToken),
            tokenOut: address(tokenB),
            amountIn: 100 ether,
            minAmountOut: 300 ether,
            nonce: 1,
            deadline: block.timestamp + 1 hours,
            chainId: block.chainid
        });
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.tokenOut = address(noReturnToken);
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);

        assertEq(noReturnToken.balanceOf(alice), 900 ether);
        assertEq(noReturnToken.balanceOf(bob), 100 ether);
    }

    // ───────────────── Transfer Failure ─────────────────

    function testRevertsWhenTransferFails() public {
        MockERC20 poorToken = new MockERC20("PoorToken");
        // alice has 0 balance — transfer will fail

        ZeroDexEscrow.Intent memory intentA = ZeroDexEscrow.Intent({
            owner: alice,
            tokenIn: address(poorToken),
            tokenOut: address(tokenB),
            amountIn: 100 ether,
            minAmountOut: 300 ether,
            nonce: 1,
            deadline: block.timestamp + 1 hours,
            chainId: block.chainid
        });
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.tokenOut = address(poorToken);
        bytes32 matchId = keccak256("match1");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        vm.expectRevert(ZeroDexEscrow.TransferFailed.selector);
        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
    }

    // ───────────────── Edge Cases ─────────────────

    function testDifferentMatchIdAllowsSameParties() public {
        _executeDefault(100 ether, 350 ether);

        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        intentA.nonce = 2;
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.nonce = 2;
        bytes32 matchId = keccak256("match2"); // different ID
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        escrow.executeSwap(intentA, intentB, 100 ether, 350 ether, matchId, sigA, sigB);
        assertTrue(escrow.nonceUsed(alice, 2));
        assertTrue(escrow.nonceUsed(bob, 2));
    }

    function testMinimalAmountSwap() public {
        ZeroDexEscrow.Intent memory intentA = _defaultIntentA();
        intentA.amountIn = 1;
        intentA.minAmountOut = 1;
        intentA.nonce = 99;
        ZeroDexEscrow.Intent memory intentB = _defaultIntentB();
        intentB.amountIn = 1;
        intentB.minAmountOut = 1;
        intentB.nonce = 100;
        bytes32 matchId = keccak256("minimal-match");
        bytes memory sigA = _sign(ALICE_PK, _intentDigest(intentA));
        bytes memory sigB = _sign(BOB_PK, _intentDigest(intentB));

        escrow.executeSwap(intentA, intentB, 1, 1, matchId, sigA, sigB);

        assertEq(tokenA.balanceOf(bob), 1);
        assertEq(tokenB.balanceOf(alice), 1);
    }
}
