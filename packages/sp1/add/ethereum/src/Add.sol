// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

struct PublicValuesStruct {
    uint256 old_root;
    uint256 new_root;
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

    /// @notice The current root state.
    uint256 public root;

    /// @notice Event emitted when root is updated.
    event RootUpdated(uint256 oldRoot, uint256 newRoot);

    constructor(address _verifier, bytes32 _addProgramVKey) {
        verifier = _verifier;
        addProgramVKey = _addProgramVKey;
        root = 0; // Initialize root to 0
    }

    /// @notice The entrypoint for verifying the proof and updating the root.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyAddProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public returns (uint256, uint256) {
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

        // Check that old_root matches current state - disabled for now
        // if (publicValues.old_root != root) {
        //    revert InvalidOldRoot(root, publicValues.old_root);
        //}

        // Update the root state
        uint256 oldRoot = root;
        root = publicValues.new_root;

        // Emit event
        emit RootUpdated(oldRoot, root);

        return (publicValues.old_root, publicValues.new_root);
    }

    /// @notice Get the current root.
    function getCurrentRoot() public view returns (uint256) {
        return root;
    }
}
