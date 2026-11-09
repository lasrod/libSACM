# Verification 0011, round 2: m0-mutation-test-order

- Date: 2026-11-09
- Verdict: **PASS**
- Row verdict: LIB-QA-012: PASS
- Verifier: `sacm-verifier` (independent; read-only). This was a proportionate round: only P1 to P4 of [round 1](0011-m0-mutation-test-order.md) were checked.
- Code inspected: `eec4748` (`git diff bfb0d99..eec4748`, `xtask/tests/repo_checks.rs` only). The later head `13fdbc0` changes only the brief.
- Evidence: ci.yml 42589365222 and mutants.yml 42589365337 at `eec4748`, both success, with 0 missed and 0 timeouts. `cargo test -p xtask` and `matrix check` pass.

| Item | Result |
|---|---|
| P1 | Met. Q21 and the "a line begins with" variant are killed. |
| P2 | Met. Q24 is killed. |
| P3 | Met. Q27 and Q29 are killed. |
| P4 | Met. The version is derived from `NEXTEST_INSTALL`, and a lone edit to it is killed. |

Accepted residual: Copilot comment 4510944803. A worker pool filled with hanging tests can still give a TIMEOUT, which fails CI. The maintainer chose not to fix it.
