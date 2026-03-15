// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.20;

contract TRIONManipulationRegistry {
    struct ManipulationRecord {
        bytes32 assetId;
        uint64 timestamp;
        uint8 kind;
        uint32 severityBps;
        bytes32 details;
    }

    address public owner;
    address public publisher;

    mapping(bytes32 => ManipulationRecord) private lastAlert;

    event ManipulationAlert(
        bytes32 indexed assetId,
        uint64 timestamp,
        uint8 kind,
        uint32 severityBps,
        bytes32 details
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

    function publishAlert(
        bytes32 assetId,
        uint64 timestamp,
        uint8 kind,
        uint32 severityBps,
        bytes32 details
    ) external onlyPublisher {
        lastAlert[assetId] = ManipulationRecord({
            assetId: assetId,
            timestamp: timestamp,
            kind: kind,
            severityBps: severityBps,
            details: details
        });
        emit ManipulationAlert(assetId, timestamp, kind, severityBps, details);
    }

    function getLastAlert(bytes32 asset) external view returns (ManipulationRecord memory) {
        return lastAlert[asset];
    }
}
