// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {Hash} from "../src/Hash.sol";
import {Poseidon} from "../src/Poseidon.sol";

contract Deploy is Script {
    function run() external returns (Poseidon poseidon) {
        uint256 pk = vm.envUint("PRIVATE_KEY"); // from .env
        vm.startBroadcast(pk);
        
        Hash hash = new Hash();
        console2.log("Hash deployed at", address(hash));
        
        poseidon = new Poseidon(address(hash));
        console2.log("Poseidon deployed at", address(poseidon));
        
        vm.stopBroadcast();
    }
}
