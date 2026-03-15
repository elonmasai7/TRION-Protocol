# Consumer Contract Examples

## Simple DeFi Gate (Solidity)
```solidity
// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.20;

interface ITRIONConsumer {
    function getSignal(bytes32 asset) external view returns (uint256 score, uint256 confidence);
}

contract TrionGate {
    ITRIONConsumer public registry;
    uint256 public minScore;

    constructor(address registryAddress, uint256 minScoreScaled) {
        registry = ITRIONConsumer(registryAddress);
        minScore = minScoreScaled;
    }

    function allow(bytes32 asset) external view returns (bool) {
        (uint256 score, uint256 confidence) = registry.getSignal(asset);
        return score >= minScore && confidence >= minScore;
    }
}
```

## SILENCE Safety Guard
```solidity
// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.20;

interface ISilenceRegistry {
    function getLastSilence(bytes32 asset) external view returns (
        bytes32 assetId,
        uint64 timestamp,
        uint64 coherence,
        uint64 confidence,
        uint8 limitingLayer,
        uint64 coherenceGap,
        uint8 trend,
        uint64 etaRecovery,
        bytes32[8] memory payload
    );
}

contract SilenceGuard {
    ISilenceRegistry public silence;

    constructor(address silenceRegistry) {
        silence = ISilenceRegistry(silenceRegistry);
    }

    function isSilent(bytes32 asset) external view returns (bool) {
        (bytes32 assetId, uint64 timestamp, , , , , , ,) = silence.getLastSilence(asset);
        return assetId != bytes32(0) && timestamp > 0;
    }
}
```
