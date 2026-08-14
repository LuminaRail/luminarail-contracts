# LuminaRail Smart Contracts — Architecture Specification

This document outlines the detailed technical design, state transition rules, storage models, authorization paths, and fee calculation rules implemented in `luminarail-contracts`.

---

## Workspace Layout

```
contracts/
├── escrow/
│   └── src/lib.rs          # Multi-party escrow logic & unit tests
├── settlement_vault/
│   └── src/lib.rs          # Settlement vault logic & unit tests
└── fee_manager/
    └── src/lib.rs          # Fee calculation logic & unit tests
```

---

## 1. Escrow Contract Architecture (`contracts/escrow`)

### State Machine Lifecycle

```
[ Uninitialized ]
       │
       ▼ (create_escrow)
   Created (0)
       │
       ▼ (fund_escrow)
    Funded (1)
       │
       ▼ (release_escrow)
   Released (2) [Terminal]
```

### Authorization Rules
- `create_escrow`: `depositor.require_auth()`
- `fund_escrow`: `depositor.require_auth()`
- `release_escrow`: `release_authority.require_auth()`

---

## 2. Settlement Vault Contract Architecture (`contracts/settlement_vault`)

### State Machine Lifecycle

```
[ Uninitialized ]
       │
       ▼ (initialize)
   Initialized (Admin Set)
       │
       ▼ (create_settlement)
    Pending (0)
       │
       ▼ (execute_settlement)
   Executed (1) [Terminal]
```

### Authorization Rules
- `initialize`: Called once; sets admin key in instance storage.
- `create_settlement`: `admin.require_auth()`
- `execute_settlement`: `admin.require_auth()` AND `source.require_auth()` for token transfer.

---

## 3. Fee Manager Architecture (`contracts/fee_manager`)

### Fee Calculation Math

```rust
// Fee calculation using integer arithmetic with overflow protection
let fee = amount
    .checked_mul(fee_bps as i128)
    .ok_or(FeeError::Overflow)?
    .checked_div(10_000)
    .ok_or(FeeError::Overflow)?;
```

### Bounds & Constraints
- `MAX_FEE_BPS`: 1000 (equivalent to 10.00%).
- Any attempt to set `fee_bps > 1000` is rejected with `FeeError::ExceedsMaxFee`.
