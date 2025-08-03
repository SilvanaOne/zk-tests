// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {Add} from "../src/Add.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct SP1ProofFixtureJson {
    uint32 value;
    uint32 oldSum;
    uint32 newSum;
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
        string memory path = string.concat(root, "/../proofs/groth16-fixture.json");
        string memory json = vm.readFile(path);
        
        SP1ProofFixtureJson memory fixture;
        fixture.value = uint32(json.readUint(".value"));
        fixture.oldSum = uint32(json.readUint(".oldSum"));
        fixture.newSum = uint32(json.readUint(".newSum"));
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

        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));

        // Contract starts with sum=0, fixture has oldSum=0, so no setup needed
        assert(addContract.getCurrentSum() == 0);

        // Create new public values with correct format (oldSum, newSum)
        bytes memory correctPublicValues = abi.encode(fixture.oldSum, fixture.newSum);
        
        (uint32 oldSum, uint32 newSum) = addContract.verifyAddProof(
            correctPublicValues, 
            vm.parseBytes(fixture.proof)
        );
        
        assert(oldSum == fixture.oldSum);
        assert(newSum == fixture.newSum);
        
        // Check that sum was updated to newSum
        assert(addContract.getCurrentSum() == fixture.newSum);
    }

    function test_MultipleAddProofs() public {
        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));

        // First addition: 0 + 3 = 3
        bytes memory publicValues1 = abi.encode(uint32(0), uint32(3));
        bytes memory proof1 = hex"dead01";
        addContract.verifyAddProof(publicValues1, proof1);
        assert(addContract.getCurrentSum() == 3);

        // Second addition: 3 + 7 = 10
        bytes memory publicValues2 = abi.encode(uint32(3), uint32(10));
        bytes memory proof2 = hex"beef02";
        addContract.verifyAddProof(publicValues2, proof2);
        assert(addContract.getCurrentSum() == 10);
    }

    function test_MultipleProofsWithSameOldSum() public {
        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));

        // First, add some value: 0 + 5 = 5
        bytes memory publicValues1 = abi.encode(uint32(0), uint32(5));
        bytes memory proof1 = hex"dead01";
        addContract.verifyAddProof(publicValues1, proof1);
        assert(addContract.getCurrentSum() == 5);
        
        // Now sum is 5, but we can still use old_sum=0 again (validation disabled)
        bytes memory publicValues2 = abi.encode(uint32(0), uint32(3));
        bytes memory proof2 = hex"beef02";
        
        // This should work since validation is disabled
        addContract.verifyAddProof(publicValues2, proof2);
        assert(addContract.getCurrentSum() == 3);
    }

    function testRevert_InvalidAddProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory realProof = vm.parseBytes(fixture.proof);
        bytes memory fakeProof = new bytes(realProof.length);

        // Create new public values with correct format (oldSum, newSum)
        bytes memory correctPublicValues = abi.encode(fixture.oldSum, fixture.newSum);
        
        addContract.verifyAddProof(correctPublicValues, fakeProof);
    }

    function test_GetCurrentSum() public {
        // Test the getCurrentSum function
        assert(addContract.getCurrentSum() == 0);
        
        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));
        
        SP1ProofFixtureJson memory fixture = loadFixture();
        
        // Create new public values with correct format (oldSum, newSum)
        bytes memory correctPublicValues = abi.encode(fixture.oldSum, fixture.newSum);
        
        addContract.verifyAddProof(
            correctPublicValues, 
            vm.parseBytes(fixture.proof)
        );
        assert(addContract.getCurrentSum() == fixture.newSum);
    }
}

contract AddPlonkTest is Test {
    using stdJson for string;

    address verifier;
    Add public addContract;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(root, "/../proofs/groth16-fixture.json");
        string memory json = vm.readFile(path);
        
        SP1ProofFixtureJson memory fixture;
        fixture.value = uint32(json.readUint(".value"));
        fixture.oldSum = uint32(json.readUint(".oldSum"));
        fixture.newSum = uint32(json.readUint(".newSum"));
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

        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));

        // Contract starts with sum=0, fixture has oldSum=0, so no setup needed
        assert(addContract.getCurrentSum() == 0);

        // Create new public values with correct format (oldSum, newSum)
        bytes memory correctPublicValues = abi.encode(fixture.oldSum, fixture.newSum);
        
        (uint32 oldSum, uint32 newSum) = addContract.verifyAddProof(
            correctPublicValues, 
            vm.parseBytes(fixture.proof)
        );
        
        assert(oldSum == fixture.oldSum);
        assert(newSum == fixture.newSum);
        
        // Check that sum was updated to newSum
        assert(addContract.getCurrentSum() == fixture.newSum);
    }

    function test_MultipleProofsWithSameOldSum() public {
        vm.mockCall(verifier, abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector), abi.encode(true));

        // First, add some value: 0 + 5 = 5
        bytes memory publicValues1 = abi.encode(uint32(0), uint32(5));
        bytes memory proof1 = hex"dead01";
        addContract.verifyAddProof(publicValues1, proof1);
        assert(addContract.getCurrentSum() == 5);
        
        // Now sum is 5, but we can still use old_sum=0 again (validation disabled)
        bytes memory publicValues2 = abi.encode(uint32(0), uint32(3));
        bytes memory proof2 = hex"beef02";
        
        // This should work since validation is disabled
        addContract.verifyAddProof(publicValues2, proof2);
        assert(addContract.getCurrentSum() == 3);
    }

    function testRevert_InvalidAddProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory realProof = vm.parseBytes(fixture.proof);
        bytes memory fakeProof = new bytes(realProof.length);

        // Create new public values with correct format (oldSum, newSum)
        bytes memory correctPublicValues = abi.encode(fixture.oldSum, fixture.newSum);
        
        addContract.verifyAddProof(correctPublicValues, fakeProof);
    }
}