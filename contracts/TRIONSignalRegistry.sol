// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.20;

import "./ITRIONConsumer.sol";

contract TRIONSignalRegistry is ITRIONConsumer {
    uint256 public constant SCORE_SCALE = 1_000_000;

    struct SignalRecord {
        bytes32 assetId;
        uint64 timestamp;
        uint64 coherence;
        uint64 confidence;
        uint32 manipulationFlags;
        bytes32[8] payload;
    }

    address public owner;
    address public publisher;

    mapping(bytes32 => SignalRecord) private lastSignal;

    event SignalPublished(bytes32 indexed assetId, uint64 timestamp, uint64 coherence, uint64 confidence, uint32 manipulationFlags, bytes32[8] payload);
    event PublisherUpdated(address indexed publisher);

    modifier onlyOwner() {
        require(msg.sender == owner, "owner only");
        _;
    }

    modifier onlyPublisher() {
        require(msg.sender == publisher, "publisher only");
        _;
    }

    constructor(address initialPublisher) {
        owner = msg.sender;
        publisher = initialPublisher;
    }

    function setPublisher(address newPublisher) external onlyOwner {
        publisher = newPublisher;
        emit PublisherUpdated(newPublisher);
    }

    function publishSignal(
        bytes32 assetId,
        uint64 timestamp,
        uint64 coherence,
        uint64 confidence,
        uint32 manipulationFlags,
        bytes32[8] calldata payload
    ) external onlyPublisher {
        lastSignal[assetId] = SignalRecord({
            assetId: assetId,
            timestamp: timestamp,
            coherence: coherence,
            confidence: confidence,
            manipulationFlags: manipulationFlags,
            payload: payload
        });

        emit SignalPublished(assetId, timestamp, coherence, confidence, manipulationFlags, payload);
    }

    function getSignal(bytes32 asset) external view override returns (uint256 score, uint256 confidence) {
        SignalRecord storage record = lastSignal[asset];
        return (record.coherence, record.confidence);
    }

    function getLastSignal(bytes32 asset) external view returns (SignalRecord memory) {
        return lastSignal[asset];
    }
}
