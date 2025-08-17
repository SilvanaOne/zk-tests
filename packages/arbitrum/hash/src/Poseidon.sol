// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

contract Poseidon {
    uint256 public hash;
    address private constant HASH_ADDRESS =
        0xE3051507DB7881fA2B3B1Fd6923211f52aFf646b;

    function setHash(uint256[] memory vec) public {
        (bool success, bytes memory data) = HASH_ADDRESS.call(
            abi.encodeWithSignature("hashList(uint256[])", vec)
        );
        require(success, "Hash call failed");
        hash = abi.decode(data, (uint256));
    }
}
