// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

contract Hash {
    function hashList(uint256[] memory vec) public pure returns (uint256) {
        uint256 sum = 0;
        for (uint256 i = 0; i < vec.length; i++) {
            sum += vec[i];
        }
        return sum;
    }
}
