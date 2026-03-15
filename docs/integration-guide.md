# Developer Integration Guide

This guide describes how DeFi protocols consume TRION signals and SILENCE events.

## On-Chain Consumption
Use `TRIONSignalRegistry` to fetch the latest signal for an asset.

- `getSignal(bytes32 asset)` returns the latest coherence and confidence values (scaled by 1e6).
- `getLastSignal(bytes32 asset)` returns the full record with payload bytes for auditability.

SILENCE signals are stored in `TRIONSilenceRegistry` via `getLastSilence(bytes32 asset)`.

## Off-Chain Consumption (API)
- `GET /signal/{asset}`
- `GET /silence/{asset}`
- `GET /manipulation-alerts`
- `WS /live-signals` (binary payload of `Signal` JSON)

## NATS Subjects
- `behavior.metrics` behavioral metrics from the indexer
- `behavior.alerts` manipulation alerts
- `trion.signals` final signals or SILENCE

## Configuration
Environment variables (defaults shown):
- `TRION_NATS_URL=nats://nats:4222`
- `TRION_POSTGRES_URL=postgres://trion:trion@postgres:5432/trion`
- `TRION_SIGNAL_REGISTRY=0x0000000000000000000000000000000000000000`
- `TRION_SILENCE_REGISTRY=0x0000000000000000000000000000000000000000`
- `TRION_COHERENCE_THRESHOLD=0.6`
- `TRION_LAYER_WEIGHTS=0.25,0.2,0.2,0.2,0.15`
- `TRION_M_MOAT=0.00005`
- `TRION_EVM_RPC=http://localhost:8545`
- `TRION_EVM_PRIVATE_KEY=...`
- `TRION_EVM_CHAIN_ID=2000`

## On-Chain Publishing (Outbox)
The signal publisher writes a `trion_evm_outbox` row per signal with the target registry, method, and 256-byte payload. A relayer can read this table and submit transactions to the Polkadot Hub EVM endpoint.
