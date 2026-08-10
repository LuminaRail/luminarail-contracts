# Security Policy — LuminaRail Smart Contracts

Smart contract security is essential for LuminaRail as an on-chain settlement infrastructure platform on Stellar.

---

## Smart Contract Security Principles

1. **Access Control & Authorization**: Every privileged function must explicitly verify invoker authorization using `address.require_auth()`.
2. **Reentrancy & Storage Safety**: Soroban storage keys (`instance`, `persistent`, `temporary`) must follow explicit lifecycle and ttl management.
3. **No Private Data On-Chain**: Never store user PII, bank account details, or raw secrets inside Soroban contract storage.
4. **Mainnet Safeguards**: Contract WASM builds must be deterministic and thoroughly audited prior to any mainnet deployment.

---

## Reporting Vulnerabilities

Report vulnerabilities to `security@luminarail.org`. Do not disclose unpatched smart contract vulnerabilities in public issues.
