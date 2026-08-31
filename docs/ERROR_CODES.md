# LuminaRail Smart Contracts — Error Code Reference & Mapping

This document provides a centralized reference mapping custom Soroban `#[contracterror]` enum variants, numeric code values, panic conditions, and recommended resolution steps for client & backend integration.

---

## 1. Escrow Contract Error Codes (`contracts/escrow`)

| Code | Enum Variant | Cause / Panic Condition | Resolution |
| :--- | :--- | :--- | :--- |
| `1` | `AlreadyInitialized` | Attempted to re-initialize an active escrow instance. | Verify escrow instance lifecycle before re-invoking. |
| `2` | `Unauthorized` | Invoker address failed `require_auth()` signature verification. | Ensure transaction is signed by the required `depositor` or `release_authority`. |
| `3` | `InvalidAmount` | Token amount is `0` or negative (`<= 0`). | Pass positive token amount integer (`> 0`). |
| `4` | `NotFound` | Requested `escrow_id` does not exist in persistent storage. | Check `escrow_id` parameter or verify creation transaction. |
| `5` | `AlreadyExists` | `escrow_id` is already registered in contract storage. | Use unique incremental `escrow_id` (e.g. UUID hash or DB sequence). |
| `6` | `InvalidState` | Invalid state transition (e.g., funding a non-`Created` escrow or releasing an un-`Funded` escrow). | Ensure state machine follows `Created -> Funded -> Released`. |
| `7` | `InvalidFee` | Fee amount configuration exceeds boundary limits. | Verify fee percentage configuration. |
| `8` | `AlreadyFunded` | Re-funding attempt on an already funded escrow. | Idempotently return existing escrow status. |
| `9` | `AlreadyReleased` | Re-releasing attempt on an already released escrow. | Return terminal `Released` status. |
| `10` | `Overflow` | Arithmetic calculation integer overflow (`i128`). | Validate input amounts against `i128::MAX`. |

---

## 2. Settlement Vault Contract Error Codes (`contracts/settlement_vault`)

| Code | Enum Variant | Cause / Panic Condition | Resolution |
| :--- | :--- | :--- | :--- |
| `1` | `AlreadyInitialized` | `initialize()` called more than once on vault instance. | Initialize vault contract once upon initial deployment. |
| `2` | `Unauthorized` | Missing `admin` or `source` entity signature. | Provide dual authorization signatures (`admin` + `source`). |
| `3` | `InvalidAmount` | Settlement amount is `<= 0`. | Specify valid positive token amount. |
| `4` | `NotFound` | Requested `settlement_id` record does not exist. | Verify settlement creation preceding execution. |
| `5` | `AlreadyExists` | `settlement_id` already exists in storage. | Generate unique settlement ID. |
| `6` | `InvalidState` | Executing a non-`Pending` settlement (e.g. duplicate execution). | Verify status is `Pending` (0) before executing. |
| `7` | `NotInitialized` | Contract function invoked before `initialize()` set vault `Admin`. | Call `initialize(admin)` after deployment. |

---

## 3. Fee Manager Contract Error Codes (`contracts/fee_manager`)

| Code | Enum Variant | Cause / Panic Condition | Resolution |
| :--- | :--- | :--- | :--- |
| `1` | `AlreadyInitialized` | `initialize()` called more than once. | Call `initialize` only during setup. |
| `2` | `Unauthorized` | Non-admin caller attempted to set fee basis points. | Sign with configured `Admin` keypair. |
| `3` | `InvalidAmount` | Negative token amount passed to `calculate_fee`. | Provide non-negative amount (`>= 0`). |
| `7` | `InvalidFee` | Requested basis points exceed `MAX_FEE_BPS` (`1000` = 10.00%). | Set basis points in range `[0, 1000]`. |
| `8` | `NotInitialized` | `set_fee_basis_points` called prior to initialization. | Initialize contract first. |
| `10` | `Overflow` | Checked multiplication overflow (`amount * bps`). | Ensure amount bounds fit within numeric range. |
