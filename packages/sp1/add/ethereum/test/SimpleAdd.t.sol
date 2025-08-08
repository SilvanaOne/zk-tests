// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {Add} from "../src/Add.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

contract SimpleAddTest is Test {
    address verifier;
    Add public addContract;

    function setUp() public {
        verifier = address(new SP1VerifierGateway(address(1)));
        // Use the actual vkey from the fixture file
        bytes32 vkey = 0x00d509f8927a764110888179e32d4c78c2bc55fc746e5d6d513c450681d9735e;
        addContract = new Add(verifier, vkey);
    }

    function test_InitialRoot() public {
        // Test that initial root is 0
        assertEq(addContract.getCurrentRoot(), 0);
    }

    function test_GetCurrentRoot() public {
        // Test the getCurrentRoot function
        assertEq(addContract.getCurrentRoot(), 0);
    }

    function test_ValidAddProofSimple() public {
        // Mock the verifier to always return true
        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));

        // Create test public values for: old_root=0, new_root=10
        bytes memory publicValues = abi.encode(uint256(0), uint256(10));
        bytes memory proof = hex"1234567890"; // Dummy proof

        // Check initial root
        assertEq(addContract.getCurrentRoot(), 0);

        (uint256 oldRoot, uint256 newRoot) = addContract.verifyAddProof(publicValues, proof);
        
        assertEq(oldRoot, 0);
        assertEq(newRoot, 10);
        
        // Check that root was updated
        assertEq(addContract.getCurrentRoot(), 10);
    }

    function test_MultipleProofsWithSameOldRoot() public {
        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));

        // First, update root: 0 -> 5
        bytes memory publicValues1 = abi.encode(uint256(0), uint256(5));
        bytes memory proof1 = hex"1234567890";
        addContract.verifyAddProof(publicValues1, proof1);
        assertEq(addContract.getCurrentRoot(), 5);
        
        // Now root is 5, but we can still use old_root=0 again (validation disabled)
        bytes memory publicValues2 = abi.encode(uint256(0), uint256(3));
        bytes memory proof2 = hex"abcdef1234";
        
        // This should work since validation is disabled
        addContract.verifyAddProof(publicValues2, proof2);
        assertEq(addContract.getCurrentRoot(), 3);
    }
}