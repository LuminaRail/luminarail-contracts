## Summary of Changes

Brief description of what this PR introduces, fixes, or refactors in `luminarail-contracts`.

Related Contributor Issue: Closes #

---

## Type of Change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Refactoring / Test coverage addition

---

## Security & Verification Checklist

- [ ] Code formatted with `cargo fmt --check`
- [ ] Static analysis passes with `cargo clippy --all-targets -- -D warnings`
- [ ] Contract builds clean with `cargo check`
- [ ] Soroban contract tests pass via `cargo test`
- [ ] Explicit `require_auth()` checks verified for all privileged entrypoints
- [ ] Storage keys & TTL management validated
- [ ] Checked arithmetic used for all financial calculations (`checked_mul`, `checked_div`)
- [ ] Zero secrets, private keys, or seed phrases included

---

## Test Results

```bash
cargo test
# Paste execution results here
```
