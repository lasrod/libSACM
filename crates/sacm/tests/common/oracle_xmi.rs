//! The XMI oracle of LIB-SEC-004: what must hold for any input to
//! `xmi::read`.
//!
//! Shared by the `xmi_read` fuzz target (`fuzz/fuzz_targets/xmi_read.rs`,
//! included with `#[path]`) and the stable replay in `fuzz_regressions.rs`, so
//! every input the fuzzer finds is replayed in the normal test run. It
//! depends on `std` and `sacm` only.

use sacm::compare::semantic_compare;
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

/// Checks `data` as an XMI document, in [`Mode::Strict`] then
/// [`Mode::Tolerant`]. In each mode:
///
/// 1. **Read.** A refusal is formatted (`Display` and `code()`), and the
///    mode ends there.
/// 2. **Format** every diagnostic of the read, then `validate` the document
///    and format each finding.
/// 3. **Write** in the same mode. Only a strict write may refuse (its
///    refusal is formatted): a tolerant read's document always writes
///    tolerantly. The text written reads back in that mode, `==` to the
///    document, and writes again to the same text.
///
/// # Panics
///
/// When one of these does not hold, with a message starting `LIB-SEC-004`;
/// and wherever the library itself panics, which is the finding.
pub(crate) fn check_xmi(data: &[u8]) {
    for mode in [Mode::Strict, Mode::Tolerant] {
        check_in(data, mode);
    }
}

/// [`check_xmi`] in one mode.
fn check_in(data: &[u8], mode: Mode) {
    let read_options = ReadOptions::default().with_mode(mode);
    let output = match xmi::read(data, &read_options) {
        Ok(output) => output,
        Err(error) => {
            let _formatted = format!("{error} {}", error.code());
            return;
        }
    };
    for diagnostic in output.diagnostics() {
        let _formatted = diagnostic.to_string();
    }
    for finding in validation::validate(output.document()) {
        let _formatted = finding.to_string();
    }
    let write_options = WriteOptions::default().with_mode(mode);
    let text = match xmi::write(output.document(), &write_options) {
        Ok(text) => text,
        Err(error) => {
            assert!(
                mode == Mode::Strict,
                "LIB-SEC-004: a tolerant read's document does not write tolerantly: {error} ({})",
                error.code()
            );
            let _formatted = format!("{error} {}", error.code());
            return;
        }
    };
    let again = match xmi::read(text.as_bytes(), &read_options) {
        Ok(again) => again,
        Err(error) => panic!(
            "LIB-SEC-004: the {mode} output does not read back in {mode} mode: {error} ({})\n{text}",
            error.code()
        ),
    };
    if again.document() != output.document() {
        let differences = semantic_compare(output.document(), again.document());
        panic!(
            "LIB-SEC-004: the {mode} output reads back as another document: {} difference(s), \
             first: {:?}\n{text}",
            differences.len(),
            differences.iter().take(3).collect::<Vec<_>>()
        );
    }
    match xmi::write(again.document(), &write_options) {
        Ok(rewritten) => assert!(
            rewritten == text,
            "LIB-SEC-004: the {mode} output, read back, writes other bytes:\n{text}\n---\n{rewritten}"
        ),
        Err(error) => panic!("LIB-SEC-004: the {mode} output, read back, does not write: {error}"),
    }
}
