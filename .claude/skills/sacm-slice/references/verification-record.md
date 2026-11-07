# Verification NNNN — <slice name>

- Date: YYYY-MM-DD
- Verdict: **PASS**
- Verifier: `sacm-verifier` (independent; read-only)
- Commit / diff inspected: <sha or "working tree at slice close">
- Slice brief: [docs/slices/NNNN-short-name.md](../slices/NNNN-short-name.md)

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| ROW-ID | … | `test_name`, … |

## Rows left open

| Row | Status kept | Reason |
|---|---|---|

## Gates run

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo test --workspace --all-features` | pass (N tests) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` | pass |
| `cargo xtask matrix check` | pass |
| `cargo deny check` | pass / not run |

## Findings and resolutions

| Severity | Row | Finding | Resolution |
|---|---|---|---|

## Verifier's report

> Paste the verifier's final report verbatim.
