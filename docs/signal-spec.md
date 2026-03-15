# TRION Signal Specification (Level-0)

Signals are emitted as a 256-byte gas-optimized payload and mirrored in JSON for off-chain services.

## Fields
- signal_type (u8) 1=SIGNAL, 2=SILENCE
- timestamp (u64) UNIX seconds
- asset_id (bytes32)
- coherence_score (u64) fixed-point, scale 1e6
- confidence (u64) fixed-point, scale 1e6
- manipulation_flags (u32) bitmask
- limiting_layer (u8) only for SILENCE
- coherence_gap (u64) fixed-point, scale 1e6
- trend (u8) 0=Flat, 1=Up, 2=Down
- eta_recovery (u64) blocks

## Encoding Layout
Offsets (bytes):
- 0: signal_type
- 1..8: timestamp
- 9..40: asset_id
- 41..48: coherence_score
- 49..56: confidence
- 57..60: manipulation_flags
- 61: limiting_layer
- 62..69: coherence_gap
- 70: trend
- 71..78: eta_recovery
- 79..255: reserved (zeros)

## Manipulation Flags
- 1 << 0: WASH_TRADING
- 1 << 1: COORDINATED_PUMP
- 1 << 2: ORACLE_ATTACK_ATTEMPT
- 1 << 3: SYBIL_LIQUIDITY
- 1 << 4: GOVERNANCE_CAPTURE
- 1 << 5: MEV_EXTRACTION_SUSTAINED
- 1 << 6: FAKE_VOLUME_PROTOCOL
