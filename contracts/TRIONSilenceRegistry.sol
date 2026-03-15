// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.20;

contract TRIONSilenceRegistry {
    struct SilenceRecord {
        bytes32 assetId;
        uint64 timestamp;
        uint64 coherence;
        uint64 confidence;
        uint8 limitingLayer;
        uint64 coherenceGap;
        uint8 trend;
        uint64 etaRecovery;
        bytes32[8] payload;
    }

    address public owner;
    address public publisher;

    mapping(bytes32 => SilenceRecord) private lastSilence;

    event SilencePublished(
        bytes32 indexed assetId,
        uint64 timestamp,
        uint64 coherence,
        uint64 confidence,
        uint8 limitingLayer,
        uint64 coherenceGap,
        uint8 trend,
        uint64 etaRecovery,
        bytes32[8] payload
    );

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

    function publishSilence(
        bytes32 assetId,
        uint64 timestamp,
        uint64 coherence,
        uint64 confidence,
        uint8 limitingLayer,
        uint64 coherenceGap,
        uint8 trend,
        uint64 etaRecovery,
        bytes32[8] calldata payload
    ) external onlyPublisher {
        lastSilence[assetId] = SilenceRecord({
            assetId: assetId,
            timestamp: timestamp,
            coherence: coherence,
            confidence: confidence,
            limitingLayer: limitingLayer,
            coherenceGap: coherenceGap,
            trend: trend,
            etaRecovery: etaRecovery,
            payload: payload
        });

        emit SilencePublished(
            assetId,
            timestamp,
            coherence,
            confidence,
            limitingLayer,
            coherenceGap,
            trend,
            etaRecovery,
            payload
        );
    }

    function getLastSilence(bytes32 asset) external view returns (SilenceRecord memory) {
        return lastSilence[asset];
    }
}
