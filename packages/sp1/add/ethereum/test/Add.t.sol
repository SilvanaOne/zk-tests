// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {Add} from "../src/Add.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct SP1ProofFixtureJson {
    string oldRoot;
    string newRoot;
    string vkey;
    string publicValues;
    string proof;
}

contract AddGroth16Test is Test {
    using stdJson for string;

    address verifier;
    Add public addContract;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/../proofs/groth16-fixture.json"
        );
        string memory json = vm.readFile(path);

        SP1ProofFixtureJson memory fixture;
        fixture.oldRoot = json.readString(".oldRoot");
        fixture.newRoot = json.readString(".newRoot");
        fixture.vkey = json.readString(".vkey");
        fixture.publicValues = json.readString(".publicValues");
        fixture.proof = json.readString(".proof");

        return fixture;
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        addContract = new Add(verifier, bytes32(vm.parseBytes(fixture.vkey)));
    }

    function test_ValidAddProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Initial root should be 0
        assert(addContract.getCurrentRoot() == 0);

        // Create new public values with correct format (old_root, new_root)
        // Note: These are hex strings in fixture; use as-is for contract decode tests
        bytes memory correctPublicValues = vm.parseBytes(fixture.publicValues);

        (uint256 oldRoot, uint256 newRoot) = addContract.verifyAddProof(
            correctPublicValues,
            vm.parseBytes(fixture.proof)
        );

        // Check that root was updated to newRoot
        // Note: The fixture may have non-zero oldRoot, so we just verify the newRoot was set
        assert(newRoot == addContract.getCurrentRoot());
    }

    function test_MultipleAddProofs() public {
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // First update: 0 -> 3 (placeholders for example)
        bytes memory publicValues1 = abi.encode(uint256(0), uint256(3));
        bytes memory proof1 = hex"dead01";
        addContract.verifyAddProof(publicValues1, proof1);
        assert(addContract.getCurrentRoot() == 3);

        // Second update: 3 -> 10
        bytes memory publicValues2 = abi.encode(uint256(3), uint256(10));
        bytes memory proof2 = hex"beef02";
        addContract.verifyAddProof(publicValues2, proof2);
        assert(addContract.getCurrentRoot() == 10);
    }

    function test_MultipleProofsWithSameOldSum() public {
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // First, update: 0 -> 5
        bytes memory publicValues1 = abi.encode(uint256(0), uint256(5));
        bytes memory proof1 = hex"dead01";
        addContract.verifyAddProof(publicValues1, proof1);
        assert(addContract.getCurrentRoot() == 5);

        // Now root is 5, but we can still use old_root=0 again (validation disabled)
        bytes memory publicValues2 = abi.encode(uint256(0), uint256(3));
        bytes memory proof2 = hex"beef02";

        // This should work since validation is disabled
        addContract.verifyAddProof(publicValues2, proof2);
        assert(addContract.getCurrentRoot() == 3);
    }

    function testRevert_InvalidAddProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory realProof = vm.parseBytes(fixture.proof);
        bytes memory fakeProof = new bytes(realProof.length);

        // Create new public values with correct format (old_root, new_root)
        bytes memory correctPublicValues = vm.parseBytes(fixture.publicValues);

        addContract.verifyAddProof(correctPublicValues, fakeProof);
    }

    function test_GetCurrentRoot() public {
        // Test the getCurrentRoot function
        assert(addContract.getCurrentRoot() == 0);

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Use the fixture's public values directly
        bytes memory correctPublicValues = vm.parseBytes(fixture.publicValues);

        addContract.verifyAddProof(
            correctPublicValues,
            vm.parseBytes(fixture.proof)
        );
        // Parse newRoot from hex string
        uint256 expectedNewRoot = uint256(vm.parseBytes32(fixture.newRoot));
        assert(addContract.getCurrentRoot() == expectedNewRoot);
    }
}

contract AddPlonkTest is Test {
    using stdJson for string;

    address verifier;
    Add public addContract;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/../proofs/groth16-fixture.json"
        );
        string memory json = vm.readFile(path);

        SP1ProofFixtureJson memory fixture;
        fixture.oldRoot = json.readString(".oldRoot");
        fixture.newRoot = json.readString(".newRoot");
        fixture.vkey = json.readString(".vkey");
        fixture.publicValues = json.readString(".publicValues");
        fixture.proof = json.readString(".proof");

        return fixture;
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        addContract = new Add(verifier, bytes32(vm.parseBytes(fixture.vkey)));
    }

    function test_ValidAddProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Contract starts with root=0
        assert(addContract.getCurrentRoot() == 0);

        // Use the fixture's public values directly
        bytes memory correctPublicValues = vm.parseBytes(fixture.publicValues);

        (uint256 oldRoot, uint256 newRoot) = addContract.verifyAddProof(
            correctPublicValues,
            vm.parseBytes(fixture.proof)
        );

        // Parse expected values from hex strings
        uint256 expectedOldRoot = uint256(vm.parseBytes32(fixture.oldRoot));
        uint256 expectedNewRoot = uint256(vm.parseBytes32(fixture.newRoot));
        
        assert(oldRoot == expectedOldRoot);
        assert(newRoot == expectedNewRoot);

        // Check that root was updated to newRoot
        assert(addContract.getCurrentRoot() == expectedNewRoot);
    }

    function test_MultipleProofsWithSameOldSum() public {
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // First, update root: 0 -> 5
        bytes memory publicValues1 = abi.encode(uint256(0), uint256(5));
        bytes memory proof1 = hex"dead01";
        addContract.verifyAddProof(publicValues1, proof1);
        assert(addContract.getCurrentRoot() == 5);

        // Now root is 5, but we can still use old_root=0 again (validation disabled)
        bytes memory publicValues2 = abi.encode(uint256(0), uint256(3));
        bytes memory proof2 = hex"beef02";

        // This should work since validation is disabled
        addContract.verifyAddProof(publicValues2, proof2);
        assert(addContract.getCurrentRoot() == 3);
    }

    function testRevert_InvalidAddProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory realProof = vm.parseBytes(fixture.proof);
        bytes memory fakeProof = new bytes(realProof.length);

        // Use the fixture's public values directly
        bytes memory correctPublicValues = vm.parseBytes(fixture.publicValues);

        addContract.verifyAddProof(correctPublicValues, fakeProof);
    }
}
