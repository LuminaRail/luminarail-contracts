# LuminaRail Smart Contracts (`luminarail-contracts`)

> **"Open financial rails for Stellar."**

`luminarail-contracts` contains the Soroban smart contracts written in Rust for LuminaRail's settlement infrastructure on the Stellar network.

> [!CAUTION]
> **TESTNET DEVELOPMENT CONTRACTS**: These smart contracts are strictly for **Stellar Testnet** development, testing, and simulation. They must NOT be treated as production-ready custody infrastructure or deployed to Mainnet.

---

## Workspace Contracts

```
contracts/
├── escrow/            # Multi-party conditional escrow (CREATED -> FUNDED -> RELEASED)
├── settlement_vault/  # Settlement coordination primitive (PENDING -> EXECUTED)
└── fee_manager/       # Protocol fee calculation, bounds enforcement & BPS rules
```

---

## Contract Architecture & Primitives

### 1. Escrow Contract (`contracts/escrow`)
- **State Machine**: `Created` (0) -> `Funded` (1) -> `Released` (2)
- **Authorization**: `depositor.require_auth()` for creation and funding; `release_authority.require_auth()` for release.
- **Storage**: Persistent storage keyed by `DataKey::Escrow(escrow_id)`.
- **Asset Handling**: Interacts with standard Soroban Stellar Asset contract using `token::Client`.

### 2. Settlement Vault Contract (`contracts/settlement_vault`)
- **State Machine**: `Pending` (0) -> `Executed` (1)
- **Authorization**: Single initialization via `initialize()`. `admin.require_auth()` for settlement creation and execution. Both `admin` and `source` authorize token transfer on execution (`source.require_auth()`).
- **Storage**: Instance storage for `Admin`; Persistent storage keyed by `DataKey::Settlement(settlement_id)`.

### 3. Fee Manager Contract (`contracts/fee_manager`)
- **Fee Rules**: BPS-based calculation (`amount * basis_points / 10_000`).
- **Limits**: `MAX_FEE_BPS = 1000` (capped at 10.00%). BPS updates require `admin.require_auth()`.
- **Arithmetic Safety**: Uses `checked_mul` and `checked_div` to prevent integer overflow errors.
- **Storage**: Instance storage for `Admin` and `FeeBps`.

---

## Backend Integration Boundary

The LuminaRail backend interacts with contracts through the Stellar SDK / Soroban RPC layer:

```
Order (SETTLEMENT_PENDING)
       ↓
Backend Settlement Engine
       ↓
Stellar SDK / Soroban RPC
       ↓
Soroban Settlement Contract
       ↓
Stellar Transaction Submission
       ↓
Stellar Ledger Finality & Confirmation
       ↓
Order (COMPLETED / SETTLED)
```

Orders in the backend transition to `SETTLEMENT_PENDING` after payment verification and remain pending until on-chain ledger confirmation is received.

---

## Development & Testing Workflow

Prerequisites: Rust 1.80+ with `wasm32-unknown-unknown` target.

```bash
# Add WebAssembly target
rustup target add wasm32-unknown-unknown

# Verify code formatting
cargo fmt --check

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Compile contracts
cargo check

# Run Soroban unit tests
cargo test
```

---

## License

[MIT License](./LICENSE)