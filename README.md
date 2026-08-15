# LuminaRail Smart Contracts (`luminarail-contracts`)

> **Open financial rails for Stellar.**

`luminarail-contracts` contains the Soroban smart contracts written in Rust for LuminaRail's cross-border settlement infrastructure on the Stellar network.

> [!CAUTION]
> **TESTNET DEVELOPMENT CONTRACTS**: These smart contracts are designed strictly for **Stellar Testnet** development, testing, and simulation. They must NOT be treated as production-ready mainnet custody infrastructure or deployed to Mainnet without comprehensive security audits.

---

## Overview

### What the Project Does
`luminarail-contracts` provides on-chain financial primitives implemented as Soroban smart contracts. It manages multi-party conditional escrows, coordinates on-chain settlement executions, and calculates protocol service fees on the Stellar Testnet.

### The Problem It Solves
Off-chain cross-border settlements often rely on centralized single-party escrow services, exposing users to counterparty risk, lack of transaction proof, and opaque fee calculation logic. `luminarail-contracts` provides:
- **Trustless Settlement Coordination**: Smart contracts enforce state machine transitions on-chain.
- **Cryptographic Authorization**: Standardized Soroban `require_auth()` checks ensure only authorized depositors, admins, or release authorities can invoke state changes.
- **Capped Protocol Fees**: Protocol fee calculations enforce strict BPS rules and checked arithmetic to eliminate overflow vulnerability risks.

### Ecosystem Purpose
`luminarail-contracts` acts as the decentralized settlement layer for LuminaRail. The backend engine (`luminarail-backend`) submits transaction invocations to these contracts, which execute on the Stellar blockchain ledger.

### Who Can Contribute
Rust developers with experience in smart contracts, WebAssembly (Wasm), Soroban SDK, cryptography, or security auditing are invited to contribute.

---

## Features & Workspace Contracts

The repository is structured as a Cargo workspace with three primary smart contracts:

```
contracts/
├── escrow/           # Multi-party conditional escrow contract
├── settlement_vault/ # On-chain settlement coordination primitive
└── fee_manager/      # Protocol fee calculation & BPS bounds manager
```

### 1. Escrow Contract (`contracts/escrow`)
- **State Machine**: `Created` (0) → `Funded` (1) → `Released` (2)
- **Authorization Enforcement**: 
  - `depositor.require_auth()` required for escrow creation and funding.
  - `release_authority.require_auth()` required to release locked funds to the beneficiary.
- **Storage**: Persistent storage keyed by `DataKey::Escrow(escrow_id)`.
- **Token Integration**: Interacts with Stellar Asset contracts using the standard `token::Client` Soroban interface.

### 2. Settlement Vault Contract (`contracts/settlement_vault`)
- **State Machine**: `Pending` (0) → `Executed` (1)
- **Authorization Enforcement**:
  - `initialize()` ensures one-time initialization of the contract administrator.
  - `admin.require_auth()` required for settlement creation.
  - Both `admin` and `source` authorize token transfer execution (`source.require_auth()`).
- **Storage**: Instance storage for `Admin`; Persistent storage keyed by `DataKey::Settlement(settlement_id)`.
- **Safety Checks**: Double-execution protection and zero-amount transfer rejection.

### 3. Fee Manager Contract (`contracts/fee_manager`)
- **Basis Points (BPS) Calculation**: Fee calculated as `(amount * basis_points) / 10,000`.
- **Strict Caps**: Enforces `MAX_FEE_BPS = 1000` (capped at 10.00%). BPS updates require `admin.require_auth()`.
- **Arithmetic Safety**: Uses Rust `checked_mul` and `checked_div` to protect against integer overflows.
- **Storage**: Instance storage for `Admin` and `FeeBps`.

---

## Architecture

```
Backend Settlement Engine (luminarail-backend)
                 │
                 ▼ (Soroban RPC / Transaction Submission)
                 │
  ┌──────────────┴──────────────────────────────────────┐
  │              Soroban Smart Contracts                │
  │                                                     │
  │  ┌────────────────────┐    ┌─────────────────────┐  │
  │  │  SettlementVault   │    │     FeeManager      │  │
  │  │  (Pending→Executed)│    │ (Capped 1000 BPS)   │  │
  │  └─────────┬──────────┘    └─────────────────────┘  │
  │            │                                        │
  │            ▼                                        │
  │  ┌────────────────────┐                             │
  │  │   Escrow Contract  │                             │
  │  │ (Created→Funded→   │                             │
  │  │      Released)     │                             │
  │  └────────────────────┘                             │
  └─────────────────────────────────────────────────────┘
                 │
                 ▼
     Stellar Ledger State Finality
```

### Storage Key Schemas

```rust
pub enum DataKey {
    Admin,                           // Instance storage
    FeeBps,                          // Instance storage
    Escrow(BytesN<32>),             // Persistent storage
    Settlement(BytesN<32>),          // Persistent storage
}
```

---

## Backend Integration Boundary

The LuminaRail backend interacts with contracts through Soroban RPC and `@stellar/stellar-sdk`:

```
Order (SETTLEMENT_PENDING)
       │
       ▼
Backend Settlement Engine (SettlementWorker)
       │
       ▼
Soroban RPC Simulation & Submission
       │
       ▼
Soroban Contract Invocation (create_settlement)
       │
       ▼
Stellar Ledger Finality Confirmation
       │
       ▼
Order Status (COMPLETED)
```

---

## Tech Stack

- **Language**: Rust 1.80+ (2021 Edition)
- **Compilation Target**: `wasm32-unknown-unknown`
- **SDK**: Soroban SDK `22.0.1`
- **Crypto Library**: `ed25519-dalek` 2.2.0

---

## Getting Started

### Prerequisites
- Rust 1.80 or higher
- `wasm32-unknown-unknown` target installed via Rustup

### Installation & Build Commands

1. **Add Wasm Target**:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. **Verify Code Formatting**:
   ```bash
   cargo fmt --check
   ```

3. **Run Clippy Linter**:
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

4. **Compile Workspace Contracts**:
   ```bash
   cargo check
   ```

5. **Build WebAssembly Binaries**:
   ```bash
   cargo build --target wasm32-unknown-unknown --release
   ```

---

## Testing

The workspace includes 15 unit tests across the three contracts, testing happy-path lifecycles, duplicate ID rejections, authorization boundaries, zero-amount handling, and arithmetic overflow protection.

```bash
# Run all Soroban unit tests
cargo test
```

### Test Breakdown
- **`contracts/escrow`**: 5 unit tests verifying lifecycle, double-funding rejection, double-release rejection, zero-amount rejection, and duplicate ID rejection.
- **`contracts/fee_manager`**: 5 unit tests verifying initialization, BPS fee calculations, max fee cap enforcement, zero-amount calculations, and integer overflow protection.
- **`contracts/settlement_vault`**: 5 unit tests verifying lifecycle, double-initialization rejection, duplicate settlement rejection, zero-amount rejection, and double-execution rejection.

---

## Deployment Guidelines (Stellar Testnet)

To deploy compiled `.wasm` contracts to Stellar Testnet using the Soroban CLI:

```bash
# 1. Build release Wasm binaries
cargo build --target wasm32-unknown-unknown --release

# 2. Install Soroban CLI (if not installed)
cargo install --locked soroban-cli

# 3. Deploy Wasm bytecodes to Testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/settlement_vault.wasm \
  --source-account <signer_identity> \
  --network testnet
```

---

## Contributing

1. **Fork & Clone** the repository.
2. **Create a Feature Branch**: `git checkout -b feature/my-contract-feature`
3. **Write Code & Unit Tests**: Implement clean Rust code adhering to Soroban best practices.
4. **Format & Lint**: Run `cargo fmt --check` and `cargo clippy`.
5. **Run Unit Tests**: Ensure `cargo test` passes cleanly.
6. **Commit Changes**: Commit with descriptive messages (e.g., `feat(escrow): add event emission`).
7. **Open Pull Request**: Submit PR against `develop`.

---

## Good First Contributions

- **Contract Events**: Add structured Soroban event emissions to contract invocations.
- **Unit Tests**: Add edge-case test coverage for storage TTL extensions.
- **Documentation**: Improve inline docstrings (`///`) for public contract functions.
- **Soroban CLI Scripts**: Add developer helper scripts for testnet deployment and initialization.

---

## Issue Guidelines

Please format bug reports and suggestions clearly:
- **Title**: Concise description of the issue or improvement.
- **Description**: Technical details and context.
- **Steps to Reproduce**: Reproduction details or failing test cases.
- **Acceptance Criteria**: Clear requirements for completion.

---

## Security

- **Private Keys**: Never commit secret seed keys (`S...`), private keys, or passwords.
- **Checked Math**: Always use checked arithmetic (`checked_add`, `checked_mul`) when computing token amounts or fees.
- **Authorization**: Ensure all state mutations enforce appropriate `.require_auth()` checks.
- **Security Policy**: See [SECURITY.md](file:///home/whiteghost/LuminaRail/luminarail-contracts/SECURITY.md) for vulnerability disclosure guidelines.

---

## License

This project is licensed under the [MIT License](./LICENSE).

---

## Project Status

- **Current Status**: Testnet Development Smart Contracts
- **Target Network**: Stellar Testnet