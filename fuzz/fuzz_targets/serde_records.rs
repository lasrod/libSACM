//! Fuzzes the JSON records of the `serde` feature: each round-trips, and a
//! deserialized operation checks and applies without a panic (LIB-SEC-004).
//! The oracle is shared with the stable replay in
//! `crates/sacm/tests/fuzz_regressions.rs`.

#![no_main]

#[allow(
    dead_code,
    reason = "the oracle module is shared with the stable replay test"
)]
#[path = "../../crates/sacm/tests/common/oracle_serde.rs"]
mod oracle_serde;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    oracle_serde::check_serde(data);
});
