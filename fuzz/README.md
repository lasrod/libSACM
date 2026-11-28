# Fuzzing libSACM

Two [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets (LIB-SEC-004,
[ADR 0046](../docs/adr/0046-quality-tooling.md) D2):

| Target | Input | Oracle |
|---|---|---|
| `xmi_read` | any bytes, as an XMI document | `crates/sacm/tests/common/oracle_xmi.rs` |
| `serde_records` | any bytes, as JSON of the `serde` records | `crates/sacm/tests/common/oracle_serde.rs` |

`xmi_read` reads the input in strict and in tolerant mode, formats every diagnostic and
validation finding, writes the document in the same mode, and reads the output back: it
must be equal and write the same bytes again. `serde_records` deserializes each record
type, serializes it back, and checks and applies each deserialized operation.

The targets include the oracles by path. The stable replay test
`crates/sacm/tests/fuzz_regressions.rs` runs the same oracles in every `cargo test`, over
the regression inputs, every fixture and generated mutations of them.

## Why this crate is outside the workspace

libfuzzer-sys carries the NCSA licence, which `deny.toml` does not allow in distributed
code. This crate has its own `[workspace]` and `Cargo.lock`, is never published, and
`cargo deny check` at the root does not see it.

## Running

Linux or macOS, with the pinned nightly:

```sh
rustup toolchain install nightly-2026-11-25
cargo +nightly-2026-11-25 install cargo-fuzz --locked --version 0.13.2
cd fuzz
mkdir -p corpus/xmi_read corpus/serde_records
cargo +nightly-2026-11-25 fuzz run xmi_read corpus/xmi_read ../crates/sacm/tests/data \
  -- -dict=xmi.dict -max_total_time=60 -max_len=65536 -timeout=10 -rss_limit_mb=2048
cargo +nightly-2026-11-25 fuzz run serde_records corpus/serde_records \
  ../crates/sacm/tests/data/golden/serde \
  -- -max_total_time=30 -max_len=16384 -timeout=10 -rss_limit_mb=2048
```

The first directory is the corpus libFuzzer writes to; the others are read-only seeds.
`corpus/`, `artifacts/` and `target/` are git-ignored.

## In CI

- **Pull requests:** the `fuzz (smoke)` job of `ci.yml` runs `xmi_read` for 60 seconds
  and `serde_records` for 30. It advises; it is not a required check.
- **Weekly:** `fuzz.yml` (Sundays 04:00 UTC, or on demand) runs each target for 20
  minutes, keeps the corpus in the Actions cache, and runs the property tests with 4096
  cases.

A crash, timeout or out-of-memory input is uploaded as an artifact.

## When the fuzzer finds an input

1. Reproduce it: `cargo +nightly-2026-11-25 fuzz run <target> artifacts/<target>/<file>`.
2. Minimise it if it is large: `cargo +nightly-2026-11-25 fuzz tmin <target> <file>`.
3. Add it, at most 64 KiB, as `crates/sacm/tests/data/fuzz/xmi/<name>` or
   `.../fuzz/serde/<name>`. The stable replay then fails on it in every test run.
4. Fix the bug, or file it as a matrix row.
