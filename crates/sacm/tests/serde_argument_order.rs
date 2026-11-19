//! A record's `arguments` read in any key order (LIB-API-019; ADR 0030
//! amendment 5, which relaxes R11).
//!
//! - Every value of `read-errors.json`, `write-errors.json` and
//!   `diagnostics.json`, passed through `serde_json::Value` (whose map sorts
//!   its keys: the `preserve_order` feature of `serde_json` is off), reads
//!   `==` to the value read from the golden text. At least one value really
//!   has its arguments
//!   reordered by this: the `SACM-MULT-001/too-many` diagnostic of `CLM_132`.
//! - A refusal record (`WriteError`, and a `ReadError` `Unsupported`, whose
//!   template is checked on its own path) and a diagnostic record, each with
//!   three or more arguments, read `==` to the original with their
//!   `arguments` keys written in reverse order, and hold their arguments in
//!   the template's order.
//! - An argument missing, extra, repeated (a duplicate key, written by hand)
//!   or a Text where a Count is declared is still refused as data, in the
//!   template's order and in reverse, for a refusal and a diagnostic, with a
//!   message that names the problem and the argument.
//! - A record read with reversed arguments serializes them in the template's
//!   order.

#![cfg(feature = "serde")]

use std::fmt::Debug;
use std::path::Path;

use sacm::diagnostic::{Argument, ArgumentValue, Diagnostic};
use sacm::xmi::{ReadError, WriteError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const ROW: &str = "LIB-API-019";

/// The directory of the golden JSON files.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

/// The golden file `name`, as text.
fn golden(name: &str) -> String {
    let path = Path::new(GOLDEN).join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

/// `text` read as a `T`, which it must be.
fn from_json<T: DeserializeOwned>(what: &str, text: &str) -> T {
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("{ROW}: {what}: {text} does not read: {error}"))
}

/// `value` as compact JSON.
fn compact<T: Serialize + Debug>(what: &str, value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|error| panic!("{ROW}: {what}: {value:?} does not serialize: {error}"))
}

/// The names of `arguments`, in order.
fn names(arguments: &[Argument]) -> Vec<&'static str> {
    arguments.iter().map(Argument::name).collect()
}

/// `"arguments":{…}` with `pairs` as its keys and (JSON) values, in order.
fn arguments_object(pairs: &[(&str, &str)]) -> String {
    let entries: Vec<String> = pairs
        .iter()
        .map(|(name, value)| format!("\"{name}\":{value}"))
        .collect();
    format!("\"arguments\":{{{}}}", entries.join(","))
}

/// `pairs` in reverse order.
fn reversed<'a>(pairs: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    pairs.iter().rev().copied().collect()
}

// --- The samples -------------------------------------------------------------------------

/// A refusal record with three arguments, one a Count: the
/// `UnrepresentableCharacter` value of `write-errors.json`, compact.
const UNREPRESENTABLE: &str = r#"{"type":"UnrepresentableCharacter","code":"SACM-XML-003","template":"SACM-XML-003/unrepresentable-character","arguments":{"element":"arguments","attribute":"name's content","code_point":1},"element":"arguments","character":"\u0001","attribute":"NameContent"}"#;

/// The template of [`UNREPRESENTABLE`].
const UNREPRESENTABLE_TEMPLATE: &str = "SACM-XML-003/unrepresentable-character";

/// The arguments of [`UNREPRESENTABLE`], in the template's order.
const UNREPRESENTABLE_ARGUMENTS: [(&str, &str); 3] = [
    ("element", "\"arguments\""),
    ("attribute", "\"name's content\""),
    ("code_point", "1"),
];

/// A `ReadError` `Unsupported` record with three arguments: the
/// `SACM-XMI-014/xmi-child-of-root` value of `read-errors.json`, compact.
const UNSUPPORTED: &str = r#"{"type":"Unsupported","code":"SACM-XMI-014","template":"SACM-XMI-014/xmi-child-of-root","arguments":{"element":"xmi:Extension","container":"sacm:AssuranceCasePackage","row":"SACM23-XMI-019"},"location":{"line":3,"column":3},"reason":"the XMI element \"xmi:Extension\" in the root element \"sacm:AssuranceCasePackage\" is not read in this release (a limit of this version; SACM23-XMI-019 reads it)"}"#;

/// The arguments of [`UNSUPPORTED`], in the template's order.
const UNSUPPORTED_ARGUMENTS: [(&str, &str); 3] = [
    ("element", "\"xmi:Extension\""),
    ("container", "\"sacm:AssuranceCasePackage\""),
    ("row", "\"SACM23-XMI-019\""),
];

/// A diagnostic record with six arguments, one a Count: the finding of
/// `CLM_132` in `diagnostics.json`, compact.
const TOO_MANY: &str = r#"{"code":"SACM-MULT-001","severity":"warning","requirement_id":"SACM23-VAL-011","clause":"8.6","affected":["CLM_132"],"location":null,"message":"Claim \"CLM_132\" has 2 descriptions (description:Description[0..1])","template":"SACM-MULT-001/too-many","arguments":{"class":"Claim","id":"CLM_132","count":2,"property":"description","type":"Description","multiplicity":"0..1"}}"#;

/// The template of [`TOO_MANY`].
const TOO_MANY_TEMPLATE: &str = "SACM-MULT-001/too-many";

/// The arguments of [`TOO_MANY`], in the template's order.
const TOO_MANY_ARGUMENTS: [(&str, &str); 6] = [
    ("class", "\"Claim\""),
    ("id", "\"CLM_132\""),
    ("count", "2"),
    ("property", "\"description\""),
    ("type", "\"Description\""),
    ("multiplicity", "\"0..1\""),
];

/// `record` with its arguments, given in the template's order as
/// `in_order`, replaced by `pairs`.
fn with_arguments(record: &str, in_order: &[(&str, &str)], pairs: &[(&str, &str)]) -> String {
    let from = arguments_object(in_order);
    assert!(
        record.contains(&from),
        "{ROW}: setup: {from} is not in {record}"
    );
    record.replacen(&from, &arguments_object(pairs), 1)
}

// --- Through serde_json::Value -----------------------------------------------------------

/// Every value of the golden file `name`, passed through
/// `serde_json::Value` and read again, both from the `Value` and from its
/// text with the keys sorted, is `==` to the value read from the golden
/// text. Returns the templates of the values whose arguments the `Value`
/// reordered.
fn sorted_values_read_back<T>(name: &str, arguments: impl Fn(&T) -> &[Argument]) -> Vec<String>
where
    T: DeserializeOwned + PartialEq + Debug,
{
    let text = golden(name);
    let direct: Vec<T> = from_json(name, &text);
    let values: Vec<Value> = from_json(name, &text);
    assert_eq!(
        values.len(),
        direct.len(),
        "{ROW}: setup: {name} holds {} values as Value and {} as records",
        values.len(),
        direct.len()
    );
    assert!(!direct.is_empty(), "{ROW}: setup: {name} is empty");
    let mut reordered = Vec::new();
    for (index, (value, expected)) in values.into_iter().zip(&direct).enumerate() {
        let sorted_text = compact(name, &value);
        let from_text: T = serde_json::from_str(&sorted_text).unwrap_or_else(|error| {
            panic!(
                "{ROW}: {name}, value {index}: with its keys sorted, {sorted_text} does not read: \
                 {error}"
            )
        });
        assert_eq!(
            &from_text, expected,
            "{ROW}: {name}, value {index}: with its keys sorted, it reads as another value"
        );
        let from_value: T = serde_json::from_value(value.clone()).unwrap_or_else(|error| {
            panic!("{ROW}: {name}, value {index}: as a Value, {value} does not read: {error}")
        });
        assert_eq!(
            &from_value, expected,
            "{ROW}: {name}, value {index}: as a Value, it reads as another value"
        );
        // The `Value`'s order of the argument names against the template's.
        let sorted_names: Vec<&str> = value
            .get("arguments")
            .and_then(Value::as_object)
            .map(|object| object.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if sorted_names != names(arguments(expected)) {
            let template = value
                .get("template")
                .and_then(Value::as_str)
                .unwrap_or_default();
            reordered.push(template.to_owned());
        }
    }
    reordered
}

#[test]
fn lib_api_019_every_golden_record_reads_with_its_arguments_in_sorted_key_order() {
    let mut reordered =
        sorted_values_read_back::<ReadError>("read-errors.json", |error| error.arguments());
    reordered.extend(sorted_values_read_back::<WriteError>(
        "write-errors.json",
        |error| error.arguments(),
    ));
    reordered.extend(sorted_values_read_back::<Diagnostic>(
        "diagnostics.json",
        Diagnostic::arguments,
    ));
    // Not vacuous: the Value really sorted the arguments of some record.
    assert!(
        reordered
            .iter()
            .any(|template| template == TOO_MANY_TEMPLATE),
        "{ROW}: setup: passing through serde_json::Value did not reorder the arguments of \
         {TOO_MANY_TEMPLATE} (reordered: {reordered:?}); is serde_json's preserve_order on?"
    );
}

// --- Any order reads as the template's order ---------------------------------------------

/// `record`, whose arguments in the template's order are `in_order`, reads
/// `==` to itself with them in reverse order, and holds them in the
/// template's order, with the same values.
fn assert_reverse_reads_in_template_order<T>(
    what: &str,
    record: &str,
    in_order: &[(&str, &str)],
    arguments: impl Fn(&T) -> &[Argument],
) where
    T: DeserializeOwned + PartialEq + Debug,
{
    let original: T = from_json(what, record);
    let reverse = with_arguments(record, in_order, &reversed(in_order));
    let read: T = match serde_json::from_str(&reverse) {
        Ok(read) => read,
        Err(error) => panic!(
            "{ROW}: {what}: with its arguments in reverse order, {reverse} is refused: {error}"
        ),
    };
    assert_eq!(
        read, original,
        "{ROW}: {what}: with its arguments in reverse order, it reads as another value"
    );
    let template_order: Vec<&str> = in_order.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names(arguments(&read)),
        template_order,
        "{ROW}: {what}: the arguments read in reverse order are not held in the template's order"
    );
    let values: Vec<&ArgumentValue> = arguments(&read).iter().map(Argument::value).collect();
    let expected: Vec<&ArgumentValue> = arguments(&original).iter().map(Argument::value).collect();
    assert_eq!(
        values, expected,
        "{ROW}: {what}: the values of the arguments read in reverse order"
    );
}

#[test]
fn lib_api_019_arguments_in_reverse_order_read_as_the_template_order() {
    assert_reverse_reads_in_template_order::<WriteError>(
        "a WriteError UnrepresentableCharacter record",
        UNREPRESENTABLE,
        &UNREPRESENTABLE_ARGUMENTS,
        |error| error.arguments(),
    );
    assert_reverse_reads_in_template_order::<ReadError>(
        "a ReadError Unsupported record",
        UNSUPPORTED,
        &UNSUPPORTED_ARGUMENTS,
        |error| error.arguments(),
    );
    assert_reverse_reads_in_template_order::<Diagnostic>(
        "a diagnostic record",
        TOO_MANY,
        &TOO_MANY_ARGUMENTS,
        Diagnostic::arguments,
    );
    // The Count keeps its kind whatever its place.
    let read: Diagnostic = from_json(
        "a diagnostic record in reverse",
        &with_arguments(
            TOO_MANY,
            &TOO_MANY_ARGUMENTS,
            &reversed(&TOO_MANY_ARGUMENTS),
        ),
    );
    assert_eq!(
        read.arguments()[2].value(),
        &ArgumentValue::Count(2),
        "{ROW}: the count of a diagnostic read in reverse order"
    );
}

// --- Still refused -------------------------------------------------------------------------

/// `record` with `pairs` as its arguments is refused as data, with a
/// message that holds `reason`.
fn assert_refused_with<T: DeserializeOwned + Debug>(
    what: &str,
    record: &str,
    in_order: &[(&str, &str)],
    pairs: &[(&str, &str)],
    reason: &str,
) {
    let invalid = with_arguments(record, in_order, pairs);
    match serde_json::from_str::<T>(&invalid) {
        Ok(value) => panic!("{ROW}: {what}: {invalid} was read as {value:?}"),
        Err(error) => {
            assert!(
                error.is_data(),
                "{ROW}: {what}: {invalid} is refused as malformed JSON, not as data: {error}"
            );
            assert!(
                error.to_string().contains(reason),
                "{ROW}: {what}: the refusal {error} does not say {reason:?}"
            );
        }
    }
}

/// The four faults on `record`, whose arguments in the template's order are
/// `in_order`, in that order and in reverse: `missing` left out; an argument
/// `extra` added; `repeated` given twice, as a duplicate key; and the Count
/// `count` given as text.
fn assert_faults_refused<T: DeserializeOwned + Debug>(
    what: &str,
    record: &str,
    template: &str,
    in_order: &[(&str, &str)],
    missing: &str,
    repeated: &str,
    count: &str,
) {
    for (order, pairs) in [
        ("in the template's order", in_order.to_vec()),
        ("in reverse order", reversed(in_order)),
    ] {
        let without: Vec<(&str, &str)> = pairs
            .iter()
            .copied()
            .filter(|(name, _)| *name != missing)
            .collect();
        assert_refused_with::<T>(
            &format!("{what}, {order}, without {missing:?}"),
            record,
            in_order,
            &without,
            &format!("the template {template:?} needs the argument {missing:?}"),
        );
        let mut extra = pairs.clone();
        extra.push(("extra", "\"x\""));
        assert_refused_with::<T>(
            &format!("{what}, {order}, with an extra argument"),
            record,
            in_order,
            &extra,
            &format!("the template {template:?} has no argument \"extra\""),
        );
        let mut twice = Vec::new();
        for pair in &pairs {
            twice.push(*pair);
            if pair.0 == repeated {
                twice.push(*pair);
            }
        }
        assert_refused_with::<T>(
            &format!("{what}, {order}, with {repeated:?} given twice"),
            record,
            in_order,
            &twice,
            &format!("the argument {repeated:?} is given twice"),
        );
        let as_text: Vec<(&str, &str)> = pairs
            .iter()
            .map(|&(name, value)| {
                if name == count {
                    (name, "\"2\"")
                } else {
                    (name, value)
                }
            })
            .collect();
        assert_refused_with::<T>(
            &format!("{what}, {order}, with the Count {count:?} given as text"),
            record,
            in_order,
            &as_text,
            &format!("the argument {count:?} is not of the kind Count"),
        );
    }
}

#[test]
fn lib_api_019_missing_extra_repeated_or_mistyped_arguments_are_still_refused() {
    // The valid twins read, so each refusal is the fault's.
    let _: WriteError = from_json("the refusal twin", UNREPRESENTABLE);
    let _: Diagnostic = from_json("the diagnostic twin", TOO_MANY);
    assert_faults_refused::<WriteError>(
        "a refusal record",
        UNREPRESENTABLE,
        UNREPRESENTABLE_TEMPLATE,
        &UNREPRESENTABLE_ARGUMENTS,
        "attribute",
        "element",
        "code_point",
    );
    assert_faults_refused::<Diagnostic>(
        "a diagnostic record",
        TOO_MANY,
        TOO_MANY_TEMPLATE,
        &TOO_MANY_ARGUMENTS,
        "id",
        "class",
        "count",
    );
    // A renamed argument: `attribute` left out and `atribute` given; the
    // unknown name is reported.
    let renamed: Vec<(&str, &str)> = UNREPRESENTABLE_ARGUMENTS
        .iter()
        .map(|&(name, value)| {
            if name == "attribute" {
                ("atribute", value)
            } else {
                (name, value)
            }
        })
        .collect();
    assert_refused_with::<WriteError>(
        "a refusal record with \"attribute\" renamed \"atribute\"",
        UNREPRESENTABLE,
        &UNREPRESENTABLE_ARGUMENTS,
        &renamed,
        &format!("the template {UNREPRESENTABLE_TEMPLATE:?} has no argument \"atribute\""),
    );
}

// --- Written in the template's order -----------------------------------------------------

/// `record` read with its arguments in reverse order serializes as `record`
/// read as given: its arguments in the template's order.
fn assert_written_in_template_order<T>(what: &str, record: &str, in_order: &[(&str, &str)])
where
    T: DeserializeOwned + Serialize + Debug,
{
    let reverse = with_arguments(record, in_order, &reversed(in_order));
    let read: T = match serde_json::from_str(&reverse) {
        Ok(read) => read,
        Err(error) => panic!(
            "{ROW}: {what}: with its arguments in reverse order, {reverse} is refused: {error}"
        ),
    };
    let written = compact(what, &read);
    let template_order = arguments_object(in_order);
    assert!(
        written.contains(&template_order),
        "{ROW}: {what}: read in reverse order, it writes {written}, not {template_order}"
    );
    let original: T = from_json(what, record);
    assert_eq!(
        written,
        compact(what, &original),
        "{ROW}: {what}: read in reverse order, it does not write as the original"
    );
}

#[test]
fn lib_api_019_serialization_writes_the_template_order() {
    assert_written_in_template_order::<WriteError>(
        "a WriteError UnrepresentableCharacter record",
        UNREPRESENTABLE,
        &UNREPRESENTABLE_ARGUMENTS,
    );
    assert_written_in_template_order::<ReadError>(
        "a ReadError Unsupported record",
        UNSUPPORTED,
        &UNSUPPORTED_ARGUMENTS,
    );
    assert_written_in_template_order::<Diagnostic>(
        "a diagnostic record",
        TOO_MANY,
        &TOO_MANY_ARGUMENTS,
    );
}
