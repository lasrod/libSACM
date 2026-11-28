//! The serde oracle of LIB-SEC-004: what must hold for any JSON given to the
//! record types of the `serde` feature (ADR 0030).
//!
//! Shared by the `serde_records` fuzz target
//! (`fuzz/fuzz_targets/serde_records.rs`, included with `#[path]`) and the
//! stable replay in `fuzz_regressions.rs`, which includes it only with the
//! `serde` feature. It depends on `std`, `sacm` and `serde_json` only.

use sacm::compare::Changes;
use sacm::diagnostic::Diagnostic;
use sacm::edit::{
    self, BatchError, BatchMutation, CreateArgumentPackage, CreateClaim, DanglingReference,
    EditError, Mutation, Operation,
};
use sacm::model::Document;
use sacm::xmi::{ReadError, WriteError};

/// The value `data` deserializes to as `$type`, if it does, after checking
/// that it serializes and reads back `==`.
macro_rules! round_trip {
    ($data:expr, $type:ty) => {{
        match serde_json::from_slice::<$type>($data) {
            Ok(value) => {
                let what = stringify!($type);
                let text = serde_json::to_string(&value).unwrap_or_else(|error| {
                    panic!("LIB-SEC-004: a deserialized {what} does not serialize: {error}")
                });
                match serde_json::from_str::<$type>(&text) {
                    Ok(back) => assert!(
                        back == value,
                        "LIB-SEC-004: a {what} serialized as {text} reads back as another value"
                    ),
                    Err(error) => panic!(
                        "LIB-SEC-004: a {what} serialized as {text} does not deserialize: {error}"
                    ),
                }
                Some(value)
            }
            Err(_) => None,
        }
    }};
}

/// Checks `data` as JSON of each record type, in the order `Operation`,
/// `Mutation`, `EditError`, `BatchMutation`, `BatchError`, `Diagnostic`,
/// `ReadError`, `WriteError`, `Changes` and `DanglingReference`, each as one
/// value and as a list:
///
/// - **Round trip.** When `data` deserializes, the value serializes and reads
///   back `==`.
/// - **Operations apply.** Each deserialized `Operation` goes through
///   `edit::check` and then `edit::apply` on a fresh document (the
///   `ArgumentPackage` `p` holding the Claim `c1`); the two give the same
///   answer. A list is applied in order to one document.
///
/// # Panics
///
/// When one of these does not hold, with a message starting `LIB-SEC-004`;
/// and wherever the library itself panics, which is the finding.
pub(crate) fn check_serde(data: &[u8]) {
    if let Some(operation) = round_trip!(data, Operation) {
        check_and_apply(&mut fresh_document(), operation);
    }
    if let Some(operations) = round_trip!(data, Vec<Operation>) {
        let mut doc = fresh_document();
        for operation in operations {
            check_and_apply(&mut doc, operation);
        }
    }
    round_trip!(data, Mutation);
    round_trip!(data, Vec<Mutation>);
    round_trip!(data, EditError);
    round_trip!(data, Vec<EditError>);
    round_trip!(data, BatchMutation);
    round_trip!(data, Vec<BatchMutation>);
    round_trip!(data, BatchError);
    round_trip!(data, Vec<BatchError>);
    round_trip!(data, Diagnostic);
    round_trip!(data, Vec<Diagnostic>);
    round_trip!(data, ReadError);
    round_trip!(data, Vec<ReadError>);
    round_trip!(data, WriteError);
    round_trip!(data, Vec<WriteError>);
    round_trip!(data, Changes);
    round_trip!(data, Vec<Changes>);
    round_trip!(data, DanglingReference);
    round_trip!(data, Vec<DanglingReference>);
}

/// The `ArgumentPackage` `p` holding the Claim `c1`.
fn fresh_document() -> Document {
    let mut doc = Document::new();
    edit::apply(&mut doc, CreateArgumentPackage::root("P").with_id("p"))
        .expect("LIB-SEC-004: an empty document accepts the root package p");
    edit::apply(&mut doc, CreateClaim::new("p", "C1").with_id("c1"))
        .expect("LIB-SEC-004: the package p accepts the Claim c1");
    doc
}

/// Checks and then applies `operation` to `doc`: `edit::check` answers as
/// `edit::apply` does.
fn check_and_apply(doc: &mut Document, operation: Operation) {
    let checked = edit::check(doc, &operation);
    let applied: Result<Mutation, EditError> = edit::apply(doc, operation);
    assert!(
        checked == applied.as_ref().map(drop).map_err(Clone::clone),
        "LIB-SEC-004: edit::check gives {checked:?}, edit::apply {applied:?}"
    );
}
