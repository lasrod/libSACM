//! Fuzzes `xmi::read` in both modes, and the write and read back of what it
//! reads (LIB-SEC-004). The oracle is shared with the stable replay in
//! `crates/sacm/tests/fuzz_regressions.rs`.

#![no_main]

#[allow(
    dead_code,
    reason = "the oracle module is shared with the stable replay test"
)]
#[path = "../../crates/sacm/tests/common/oracle_xmi.rs"]
mod oracle_xmi;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    oracle_xmi::check_xmi(data);
});
