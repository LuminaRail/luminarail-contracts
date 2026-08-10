# Contributing to LuminaRail Smart Contracts

Welcome to the **LuminaRail Contracts** repository. This repository houses Soroban smart contracts written in Rust for on-chain settlement, escrows, and fee management on the Stellar network.

---

## Workspace Structure

Contracts are maintained in an independent Cargo workspace:
- `contracts/settlement_vault/`: Settlement vault contract.
- `contracts/escrow/`: Time-locked / conditions escrow contract.
- `contracts/fee_manager/`: Fee distribution & calculation contract.

---

## Guidelines

1. **No Std**: Contracts must strictly compile with `#![no_std]` target to adhere to Soroban WASM runtime requirements.
2. **Testing**: Comprehensive unit tests using `soroban-sdk::testutils` must accompany all smart contract methods.
3. **No Mainnet Deployment in Foundation Phase**: Contracts in this repository are for testnet/local simulation during early phases.

---

## Development Workflow

1. Format code: `cargo fmt`
2. Run clippy: `cargo clippy --all-targets`
3. Check compilation: `cargo check`
4. Run tests: `cargo test`

---

## Branching & PRs

Work on `develop` branch or `feature/*` branches. Submit PRs against `develop`.
