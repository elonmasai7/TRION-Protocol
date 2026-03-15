# TRION Level-0 Overview

TRION is a behavioral oracle that derives valuation signals from on-chain behavior rather than price feeds. Level-0 focuses on producing a coherent behavioral score, detecting manipulation fingerprints, and publishing signals (or SILENCE) on-chain.

## Core Equation

T(t) = [C(t) >= Theta(t)] * S(t) * exp(M_moat * t)

Where:
- C(t): behavioral coherence score
- Theta(t): minimum coherence threshold
- S(t): validated signal output
- M_moat: compounding moat coefficient
- t: time in blocks

If C(t) < Theta(t), TRION emits a SILENCE signal with structured context describing the failure.

## Behavioral Layers
1. Liquidity Structure
2. Transaction Entropy
3. Wallet Network Integrity
4. Governance Stability
5. Cross-Chain Capital Flow

Each layer produces score, confidence, and trend. Weighted aggregation produces C(t).

## Data Pipeline
Block Data -> Behavioral Indexer -> Signal Engine -> Signal Publisher -> Smart Contracts

## Level-0 Components
- Rust behavioral indexer (NATS + Timescale/Postgres + Neo4j stubs)
- Rust manipulation detector
- Rust signal engine
- Rust signal publisher
- Solidity registries + interface
- Vyper validator staking and slashing
- REST + WebSocket API

## Repository Layout
- trion (shared Rust types + signal encoding)
- indexer
- signal-engine
- manipulation-detector
- signal-publisher
- contracts
- validator
- api
- docker
- docs
