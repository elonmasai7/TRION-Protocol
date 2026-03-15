// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.20;

interface ITRIONConsumer {
    function getSignal(bytes32 asset) external view returns (uint256 score, uint256 confidence);
}
