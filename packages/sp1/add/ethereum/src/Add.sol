// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

struct PublicValuesStruct {
    uint32 old_sum;
    uint32 new_sum;
}

/// @title Add.
/// @author Succinct Labs
/// @notice This contract implements a simple example of verifying the proof of computing an addition
///         and maintaining a running sum state.
contract Add {
    /// @notice The address of the SP1 verifier contract.
    /// @dev This can either be a specific SP1Verifier for a specific version, or the
    ///      SP1VerifierGateway which can be used to verify proofs for any version of SP1.
    ///      For the list of supported verifiers on each chain, see:
    ///      https://github.com/succinctlabs/sp1-contracts/tree/main/contracts/deployments
    address public verifier;

    /// @notice The verification key for the add program.
    bytes32 public addProgramVKey;

    /// @notice The current sum state.
    uint32 public sum;

    /// @notice Event emitted when sum is updated.
    event SumUpdated(uint32 oldSum, uint32 newSum);

    constructor(address _verifier, bytes32 _addProgramVKey) {
        verifier = _verifier;
        addProgramVKey = _addProgramVKey;
        sum = 0; // Initialize sum to 0
    }

    /// @notice The entrypoint for verifying the proof of an addition and updating the sum.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyAddProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public returns (uint32, uint32) {
        // Verify the proof first
        ISP1Verifier(verifier).verifyProof(
            addProgramVKey,
            _publicValues,
            _proofBytes
        );

        // Decode public values
        PublicValuesStruct memory publicValues = abi.decode(
            _publicValues,
            (PublicValuesStruct)
        );

        // Check that old_sum matches current sum state - disabled for now
        // if (publicValues.old_sum != sum) {
        //    revert InvalidOldSum(sum, publicValues.old_sum);
        //}

        // Update the sum state
        uint32 oldSum = sum;
        sum = publicValues.new_sum;

        // Emit event
        emit SumUpdated(oldSum, sum);

        return (publicValues.old_sum, publicValues.new_sum);
    }

    /// @notice Get the current sum.
    function getCurrentSum() public view returns (uint32) {
        return sum;
    }
}
