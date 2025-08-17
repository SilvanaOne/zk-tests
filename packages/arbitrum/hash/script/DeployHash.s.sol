// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {Hash} from "../src/Hash.sol";

contract DeployHash is Script {
    function run() external returns (Hash hash) {
        uint256 pk = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(pk);
        
        hash = new Hash();
        console2.log("Hash deployed at", address(hash));
        
        vm.stopBroadcast();
    }
}