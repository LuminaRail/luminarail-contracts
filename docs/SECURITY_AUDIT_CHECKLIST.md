# LuminaRail Smart Contracts — Security Audit & Threat Vector Review

This document presents a security evaluation, authorization boundary check, and threat mitigation analysis for `luminarail-contracts`.

---

## 1. Executive Security Summary

The LuminaRail contract suite (`escrow`, `settlement_vault`, `fee_manager`) operates on Stellar's Soroban WebAssembly runtime. Soroban enforces host-level cryptographic signature verification and explicit authorization trees via `.require_auth()`.

| Threat Domain | Assessment | Mitigation Implemented |
| :--- | :--- | :--- |
| **Re-entrancy Attacks** | **Not Applicable / Prevented** | Soroban host architecture isolates contract state transitions; single-thread synchronous host calls prevent external callback re-entrancy. |
| **Unauthorized Invocations** | **Mitigated** | Strict explicit `require_auth()` checks on all state-mutating entrypoints (`admin`, `depositor`, `source`, `release_authority`). |
| **Integer Overflow / Underflow** | **Mitigated** | All fee calculations and arithmetic operations use checked arithmetic (`checked_mul`, `checked_div`) and return `Error::Overflow` rather than panicking or wrapping. |
| **Replay / Duplicate State Attacks** | **Mitigated** | Strict storage key checks (`has(&key)`) reject duplicate IDs with `Error::AlreadyExists`. State machine validation enforces single-execution terminal states. |
| **Privilege Escalation** | **Mitigated** | Immutable initialization patterns prevent re-initialization (`Error::AlreadyInitialized`). Admin updates require current admin signature. |

---

## 2. Authorization Boundary Matrix

### Escrow Contract (`contracts/escrow`)
- **`create_escrow`**: Enforces `depositor.require_auth()`. Prevents unauthorized parties from creating escrow obligations under another address.
- **`fund_escrow`**: Enforces `depositor.require_auth()`. Ensures asset transfer (`transfer(depositor -> contract, amount)`) is authorized by token owner.
- **`release_escrow`**: Enforces `release_authority.require_auth()`. Only designated authority can trigger payout to beneficiary.

### Settlement Vault (`contracts/settlement_vault`)
- **`initialize`**: Enforces `admin.require_auth()`. Immutable single-call setup.
- **`create_settlement`**: Enforces `admin.require_auth()`.
- **`execute_settlement`**: Enforces **dual authorization**: `admin.require_auth()` AND `record.source.require_auth()`. Prevents admin from arbitrarily draining source funds without source approval.

### Fee Manager (`contracts/fee_manager`)
- **`initialize`**: Enforces `admin.require_auth()` with `initial_bps <= 1000`.
- **`set_fee_basis_points`**: Enforces `admin.require_auth()` with `basis_points <= 1000`. Capped at 10.00% max protocol fee.

---

## 3. Storage Footprint & Expiration Risk
- **Escrow & Vault**: Use Soroban `persistent()` storage for contract records. Persistent storage entry lifetime is extended automatically or maintained via TTL bump when accessed.
- **Instance Storage**: Used for configuration keys (`Admin`, `FeeBps`). Instance TTL is tied to contract code deployment.
