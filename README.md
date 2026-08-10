# LuminaRail Smart Contracts (`luminarail-contracts`)

> **"Open financial rails for Stellar."**

`luminarail-contracts` contains the Soroban smart contracts written in Rust for LuminaRail's settlement infrastructure on the Stellar network.

---

## Workspace Contracts

```
contracts/
├── settlement_vault/  # Manages locked funds and settlement verification
├── escrow/            # Conditional and time-locked multi-party escrows
└── fee_manager/       # Protocol fee calculation & distribution rules
```

---

## Development & Testing Workflow

Prerequisites: Rust 1.80+ with `wasm32-unknown-unknown` target.

```bash
# Add WebAssembly target
rustup target add wasm32-unknown-unknown

# Verify code formatting
cargo fmt --check

# Run linter
cargo clippy

# Compile contracts
cargo check

# Run Soroban unit tests
cargo test
```

---

## Building WASM Bytecode

```bash
cargo build --target wasm32-unknown-unknown --release
```

---

## Security & Deployment Safeguards

- Privilege checks (`require_auth`) are enforced on sensitive contract methods.
- Phase 0 foundation contracts are strictly for testnet simulation and local development.
- Mainnet deployment is prohibited in Phase 0.

---

## License

[MIT License](./LICENSE)