# Contributing to LuminaRail Smart Contracts

Thank you for your interest in contributing to **LuminaRail Smart Contracts**! LuminaRail is an open-source settlement infrastructure platform connecting local payment rails with programmable USDC stablecoin settlement on the Stellar network using Soroban smart contracts.

---

## Drips Wave & Stellar Open Source Ecosystem

LuminaRail is part of the Stellar open-source ecosystem and Drips Wave contributor program. We welcome open-source contributions from Rust developers, Web3 engineers, and smart contract auditors.

### 1. What LuminaRail Contracts Are
`luminarail-contracts` is a Cargo workspace containing Soroban WASM smart contracts written in `#![no_std]` Rust:
- **`contracts/escrow`**: Multi-party conditional escrow contract (`Created` → `Funded` → `Released`).
- **`contracts/settlement_vault`**: On-chain settlement coordination primitive (`Pending` → `Executed`).
- **`contracts/fee_manager`**: Protocol fee calculation and BPS bounds manager (capped at 1,000 BPS / 10%).

### 2. Why We Use Stellar & Soroban
- **Stellar**: Delivers fast, low-cost asset settlement.
- **Soroban**: WebAssembly smart contract engine enabling secure contract state, authorization controls (`require_auth()`), checked arithmetic, and verifiable event logs.

### 3. Which Repository Should You Work In?
- **`luminarail-contracts`** (This repository): Soroban Rust smart contracts.
- **`luminarail-frontend`**: Next.js 16 UI application for users, merchants, and order dashboards.
- **`luminarail-backend`**: Node.js, Express, PostgreSQL, Prisma API service handling Paystack webhooks, order state machines, and Soroban contract invocation.

---

## Contributor Skill Breakdown

### Good for Beginner / Intermediate Rust / Web3 Contributors
- Adding inline rustdoc comments (`///`) and error code documentation
- Writing unit tests with `soroban-sdk::testutils` for edge cases and authorization boundaries
- Enhancing Soroban event emissions (`env.events().publish(...)`)
- Structuring CLI shell scripts (`deploy.sh`) and build tooling

### Requires Deeper Soroban / Cryptography Knowledge
- Storage TTL extension policies (`extend_ttl`) and instance storage management
- Multi-contract cross-contract call integration tests
- WASM binary size optimization (`opt-level = "z"`) and memory footprint analysis
- Access control, re-entrancy, and security auditing

---

## Development Setup & Workflow

### Prerequisites
- Rust >= 1.80
- `wasm32-unknown-unknown` target installed:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

### 1. Clone & Branch Strategy
We follow a strict branching model. All work should branch off and merge into `develop`:
```bash
git checkout develop
git pull origin develop
git checkout -b feature/your-feature-name
```

### 2. Running Verification Commands
Before submitting code, all of the following commands MUST pass cleanly:
```bash
cargo fmt --check                            # Code formatting check
cargo clippy --all-targets -- -D warnings    # Clippy linter
cargo check                                  # Check workspace compilation
cargo test                                   # Run 15 unit tests
cargo build --target wasm32-unknown-unknown --release # Build release WASM binaries
```

---

## Submitting a Pull Request

1. Push your branch to GitHub:
   ```bash
   git push origin feature/your-feature-name
   ```
2. Open a Pull Request targeting the **`develop`** branch.
3. Complete the PR description detailing the problem solved, changes made, and test results.
4. Request review from maintainers.

---

## Testnet Disclaimer
Contracts in this repository are currently deployed and tested on **Stellar Testnet**. Do NOT deploy to Mainnet without an independent third-party security audit.

---

## License
This project is licensed under the [MIT License](./LICENSE).
