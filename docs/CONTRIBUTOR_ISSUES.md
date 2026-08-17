# LuminaRail Contracts — Proposed Contributor Issues

This document contains 10 repository-grounded, production-relevant contributor issues for `luminarail-contracts`.

---

### Issue 1: Standardize Soroban event emissions across Escrow and Settlement Vault contracts
- **Problem**: While `escrow` and `settlement_vault` execute state machine transitions on-chain, they lack standardized `env.events().publish(...)` topic emissions for indexing services.
- **Scope**: Define standard event topic tuples (e.g., `(Symbol::new(&env, "escrow_created"), escrow_id)`) and publish events on all state transitions in `escrow` and `settlement_vault`.
- **Acceptance Criteria**:
  - Soroban events published for creation, funding, release, and execution.
  - Unit tests verifying event publication using `env.events().all()`.
  - Zero build warnings.
- **Relevant Area**: Contracts
- **Difficulty**: Easy / Medium
- **Potential Skills**: Rust, Soroban SDK, Testing

---

### Issue 2: Add automated storage TTL extension helper methods and unit tests
- **Problem**: Soroban persistent storage entries (escrows, settlement records) expire if their Time-To-Live (TTL) is not periodically extended.
- **Scope**: Add `extend_escrow_ttl` and `extend_settlement_ttl` methods to `escrow` and `settlement_vault` using `env.storage().persistent().extend_ttl(...)`.
- **Acceptance Criteria**:
  - Dedicated TTL extension functions allowing admin/depositor to bump entry lifetime.
  - Unit tests verifying TTL extension behavior.
  - All 15+ tests pass.
- **Relevant Area**: Contracts
- **Difficulty**: Medium
- **Potential Skills**: Rust, Soroban SDK Storage, Testing

---

### Issue 3: Build multi-contract integration test suite connecting Settlement Vault and Fee Manager
- **Problem**: Contract unit tests test `escrow`, `settlement_vault`, and `fee_manager` in isolation; cross-contract invocation flow needs an end-to-end integration test.
- **Scope**: Create `contracts/settlement_vault/tests/integration_test.rs` that initializes FeeManager, calculates fee, and executes vault settlement in a single test environment.
- **Acceptance Criteria**:
  - Integration test deploying both Wasm contracts in Soroban test environment.
  - Validate fee deduction and vault settlement execution in one workflow.
  - Test passes under `cargo test`.
- **Relevant Area**: Contracts
- **Difficulty**: Medium / High
- **Potential Skills**: Rust, Soroban SDK Testutils, Cross-Contract Calls

---

### Issue 4: Enhance deploy.sh script with automated Wasm optimization and Soroban binding generation
- **Problem**: `deploy.sh` handles basic compilation and CLI deployment, but lacks automated Wasm footprint optimization (`wasm-opt`) and TypeScript binding generation (`soroban contract binding typescript`).
- **Scope**: Update `deploy.sh` script to check for `wasm-opt`, optimize compiled binaries, and generate frontend TypeScript binding types.
- **Acceptance Criteria**:
  - `deploy.sh` supports optional `--optimize` and `--bindings` flags.
  - Generates valid TypeScript client bindings output directory.
  - Clean execution on bash environments.
- **Relevant Area**: Contracts
- **Difficulty**: Easy / Medium
- **Potential Skills**: Bash, Shell Scripting, Soroban CLI, Wasm-opt

---

### Issue 5: Add comprehensive inline rustdoc documentation for all public contract methods
- **Problem**: Several public contract functions in `contracts/escrow/src/lib.rs` and `contracts/settlement_vault/src/lib.rs` lack triple-slash `///` rustdoc comments explaining parameters and panic conditions.
- **Scope**: Add clear, detailed rustdoc comments to all contract entrypoints, data structures (`DataKey`, `EscrowState`), and error enums.
- **Acceptance Criteria**:
  - 100% rustdoc coverage on public contract interface symbols.
  - `cargo doc --no-deps` builds without warnings.
  - Markdown formatting rendered cleanly.
- **Relevant Area**: Contracts
- **Difficulty**: Easy
- **Potential Skills**: Rust, Rustdoc, Technical Writing

---

### Issue 6: Document custom Error enums and map numeric codes across all contracts
- **Problem**: Smart contract errors use custom `#[contracterror]` enums (e.g. `Error::AlreadyInitialized`), but error codes are not mapped or documented in a centralized reference for backend developers.
- **Scope**: Document all error variants across `escrow`, `settlement_vault`, and `fee_manager` in `docs/ERROR_CODES.md` with numeric mappings and troubleshooting steps.
- **Acceptance Criteria**:
  - `docs/ERROR_CODES.md` table mapping enum variants, integer values, and panic scenarios.
  - Unit tests asserting expected error enum values on failed invocations.
  - Clean documentation.
- **Relevant Area**: Contracts
- **Difficulty**: Easy
- **Potential Skills**: Rust, Soroban Error Enums, Markdown

---

### Issue 7: Add edge-case test coverage for Fee Manager basis points boundary limits
- **Problem**: `fee_manager` enforces `MAX_FEE_BPS = 1000` (10%), but unit tests do not cover boundary cases such as 1 BPS (0.01%), 999 BPS, or large token amounts near `i128::MAX`.
- **Scope**: Add boundary unit tests in `contracts/fee_manager/src/test.rs` testing minimum fees, max fees, and edge amounts.
- **Acceptance Criteria**:
  - Tests verifying 1 BPS calculation, 1000 BPS calculation, and rejection of 1001 BPS.
  - Test asserting `checked_mul` overflow safety on extreme input amounts.
  - All tests pass under `cargo test`.
- **Relevant Area**: Contracts
- **Difficulty**: Easy / Medium
- **Potential Skills**: Rust, Soroban SDK, Testing

---

### Issue 8: Add unit tests for Settlement Vault duplicate settlement verification
- **Problem**: `settlement_vault` prevents duplicate settlement IDs, but coverage for concurrent/repeated execution attempts on expired settlements needs additional test cases.
- **Scope**: Extend `contracts/settlement_vault/src/test.rs` to test invalid transitions and assertion failures when re-submitting processed settlement IDs.
- **Acceptance Criteria**:
  - Unit test verifying explicit panic/error on duplicate settlement creation.
  - Unit test verifying explicit panic on double-execution of executed settlement.
  - All tests pass cleanly.
- **Relevant Area**: Contracts
- **Difficulty**: Easy
- **Potential Skills**: Rust, Soroban SDK, Testing

---

### Issue 9: Create standalone contract state inspection CLI tool for operators
- **Problem**: Inspecting persistent contract state on Stellar Testnet requires constructing raw Soroban RPC `getContractData` calls manually.
- **Scope**: Create a helper script or Rust binary `scripts/inspect-state.rs` using `soroban-cli` to decode and display escrow and settlement vault state by ID.
- **Acceptance Criteria**:
  - CLI command taking contract ID and escrow/settlement ID and outputting formatted JSON state.
  - Handles missing or expired keys gracefully.
  - Clean documentation.
- **Relevant Area**: Contracts
- **Difficulty**: Medium
- **Potential Skills**: Rust / Bash, Soroban CLI, JSON

---

### Issue 10: Conduct security audit review for admin access control and re-entrancy safety
- **Problem**: Contracts must be thoroughly audited for access control risks, such as administrative privilege escalation or state mutation order vulnerabilities.
- **Scope**: Perform a formal code-level security review of all three contracts, document authorization boundaries, and add regression tests for unauthorized invocation attempts.
- **Acceptance Criteria**:
  - `docs/SECURITY_AUDIT_CHECKLIST.md` detailing authorization checks and state ordering.
  - Regression tests for unauthorized `admin` or `depositor` calls.
  - All 15+ tests pass cleanly.
- **Relevant Area**: Contracts
- **Difficulty**: Medium / High
- **Potential Skills**: Rust, Smart Contract Security, Soroban SDK
