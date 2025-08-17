// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {Hash} from "../src/Hash.sol";
import {Poseidon} from "../src/Poseidon.sol";

contract HashTest is Test {
    Hash public hashContract;
    Poseidon public poseidonContract;

    function setUp() public {
        hashContract = new Hash();
        poseidonContract = new Poseidon(address(hashContract));
    }

    function test_HashList() public {
        uint256[] memory vec = new uint256[](3);
        vec[0] = 1;
        vec[1] = 2;
        vec[2] = 3;
        
        uint256 result = hashContract.hashList(vec);
        assertEq(result, 6);
    }

    function test_PoseidonHash() public {
        uint256[] memory vec = new uint256[](3);
        vec[0] = 1;
        vec[1] = 2;
        vec[2] = 3;
        
        poseidonContract.hash(vec);
        assertEq(poseidonContract.hash(), 6);
    }

    function testFuzz_HashList(uint256[] memory vec) public {
        uint256 expectedSum = 0;
        for (uint256 i = 0; i < vec.length; i++) {
            expectedSum += vec[i];
        }
        
        uint256 result = hashContract.hashList(vec);
        assertEq(result, expectedSum);
    }

    function testFuzz_PoseidonHash(uint256[] memory vec) public {
        uint256 expectedSum = 0;
        for (uint256 i = 0; i < vec.length; i++) {
            expectedSum += vec[i];
        }
        
        poseidonContract.hash(vec);
        assertEq(poseidonContract.hash(), expectedSum);
    }
}
