# LuminaRail Contracts — Event Emission Specification

This document provides a technical specification of the standardized Soroban events emitted by `luminarail-contracts`.

Off-chain indexing services, backend notification system listeners, and audit trail monitors rely on these events to observe contract state machine transitions in real time without polling contract storage keys.

---

## 1. Event Publication Architecture

All contract events are published using Soroban's native event interface:

```rust
env.events().publish(topics, data);
```

### Event Properties:
- **Topics**: A 2-tuple `(Symbol, u64)` where Topic 0 is the event type symbol (e.g. `"escrow_created"`) and Topic 1 is the entity identifier (`escrow_id` or `settlement_id`).
- **Data Payload**: A tuple containing key details of the transition (addresses, asset contract address, amount).
- **Execution Order**: Events are published only after state persistence and token transfers have succeeded. In case of panic or transaction rollback, no events are recorded on-chain.

---

## 2. Escrow Contract Events (`contracts/escrow`)

### `escrow_created`
Emitted when a new escrow agreement is initialized in `Created` status.

- **Topic 0**: `Symbol::new(&env, "escrow_created")`
- **Topic 1**: `escrow_id: u64`
- **Payload**: `(depositor: Address, beneficiary: Address, asset: Address, amount: i128)`
- **Trigger**: Called in `create_escrow` after validating amount and persisting the record.

### `escrow_funded`
Emitted when token assets are transferred from the depositor to the Escrow contract, advancing status to `Funded`.

- **Topic 0**: `Symbol::new(&env, "escrow_funded")`
- **Topic 1**: `escrow_id: u64`
- **Payload**: `(depositor: Address, amount: i128)`
- **Trigger**: Called in `fund_escrow` after token transfer to contract address and state update.

### `escrow_released`
Emitted when escrowed token assets are released from the Escrow contract to the beneficiary, advancing status to `Released`.

- **Topic 0**: `Symbol::new(&env, "escrow_released")`
- **Topic 1**: `escrow_id: u64`
- **Payload**: `(beneficiary: Address, amount: i128)`
- **Trigger**: Called in `release_escrow` after token transfer to beneficiary address and state update.

---

## 3. Settlement Vault Events (`contracts/settlement_vault`)

### `settlement_created`
Emitted when an institutional settlement instruction is registered in `Pending` status.

- **Topic 0**: `Symbol::new(&env, "settlement_created")`
- **Topic 1**: `settlement_id: u64`
- **Payload**: `(source: Address, destination: Address, asset: Address, amount: i128)`
- **Trigger**: Called in `create_settlement` after admin authorization check and state persistence.

### `settlement_executed`
Emitted when a pending settlement instruction is atomically executed, transferring tokens from source to destination and setting status to `Executed`.

- **Topic 0**: `Symbol::new(&env, "settlement_executed")`
- **Topic 1**: `settlement_id: u64`
- **Payload**: `(source: Address, destination: Address, amount: i128)`
- **Trigger**: Called in `execute_settlement` after dual auth verification, token transfer, and state update.

---

## 4. Summary Table

| Contract | Event Topic | Entity ID | Payload Fields |
| :--- | :--- | :--- | :--- |
| `escrow` | `"escrow_created"` | `escrow_id` (`u64`) | `(depositor, beneficiary, asset, amount)` |
| `escrow` | `"escrow_funded"` | `escrow_id` (`u64`) | `(depositor, amount)` |
| `escrow` | `"escrow_released"` | `escrow_id` (`u64`) | `(beneficiary, amount)` |
| `settlement_vault` | `"settlement_created"` | `settlement_id` (`u64`) | `(source, destination, asset, amount)` |
| `settlement_vault` | `"settlement_executed"` | `settlement_id` (`u64`) | `(source, destination, amount)` |
