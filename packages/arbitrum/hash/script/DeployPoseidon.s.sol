// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {Poseidon} from "../src/Poseidon.sol";

contract DeployPoseidon is Script {
    function run() external returns (Poseidon poseidon) {
        uint256 pk = vm.envUint("PRIVATE_KEY");
        
        vm.startBroadcast(pk);
        
        poseidon = new Poseidon();
        console2.log("Poseidon deployed at", address(poseidon));
        
        vm.stopBroadcast();
    }
}