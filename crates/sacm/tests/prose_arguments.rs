//! Prose arguments are marked as prose (LIB-DIAG-004; ADR 0030 amendment 6).
//!
//! An argument whose value is English that the crate or its XML parser
//! wrote, such as a parser error or an encoding label, is an
//! `ArgumentValue::Prose`, not `Text`, so a caller that words messages in
//! another language can tell it from a name, id or value.
//!
//! - `lib_diag_004_a_prose_value_is_neither_text_nor_a_count` checks the
//!   accessors, `Display`, equality and order of `ArgumentValue::Prose`, and
//!   the `Display` of a prose `Argument` a refusal holds.
//! - `lib_diag_004_each_prose_argument_a_refusal_reaches_is_prose` reads or
//!   writes one input for each of the five prose arguments a refusal reaches
//!   through the public API, and checks every argument by name and variant
//!   and the whole `Display`, byte for byte as `message_templates.rs` pins
//!   it. The other three, `error` of `SACM-SEC-002/unheld-*`, have no input
//!   (`message_templates.rs`, `NO_INPUT`); unit tests in `xmi/roots.rs` check
//!   the arguments that `unheld` and `unheld_outside` build.
//! - With `serde`, `lib_diag_004_a_prose_argument_is_a_json_string_and_reads_back_as_prose`
//!   checks that a prose argument is a JSON string, that the JSON of a
//!   refusal holding one is unchanged, that it reads back as `Prose`, and
//!   that a number for it is refused.
//!
//! Which template arguments are prose, and the catalog's marks, are
//! `xtask/tests/message_templates.rs`'s.

use std::cmp::Ordering;

use sacm::diagnostic::{Argument, ArgumentValue};
use sacm::edit::{self, CreateArgumentPackage};
use sacm::model::{Document, LangString};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "LIB-DIAG-004";

/// The arguments of a refusal, each its name and value.
fn named(arguments: &[Argument]) -> Vec<(&'static str, ArgumentValue)> {
    arguments
        .iter()
        .map(|argument| (argument.name(), argument.value().clone()))
        .collect()
}

fn prose(text: &str) -> ArgumentValue {
    ArgumentValue::Prose(text.to_owned())
}

fn text(text: &str) -> ArgumentValue {
    ArgumentValue::Text(text.to_owned())
}

/// The refusal of a strict read of `input`.
fn strict_refusal(what: &str, input: &[u8]) -> ReadError {
    match xmi::read(input, &ReadOptions::default().with_mode(Mode::Strict)) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read accepts the input, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// A document holding the root ArgumentPackage `arguments`, whose name holds
/// U+0001.
fn unrepresentable_name() -> Document {
    let mut document = Document::new();
    if let Err(error) = edit::apply(
        &mut document,
        CreateArgumentPackage::root(LangString::new("bad\u{1}")).with_id("arguments"),
    ) {
        panic!("{ROW}: setup: the package is not created: {error}");
    }
    document
}

/// The refusal of a strict write of [`unrepresentable_name`].
fn unrepresentable_refusal() -> WriteError {
    match xmi::write(&unrepresentable_name(), &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: a strict write writes a name holding U+0001: {text}"),
        Err(error) => error,
    }
}

/// `text` encoded as UTF-16 LE, without a byte order mark.
fn utf16le(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// `<r/>` encoded as UTF-32 BE, after its byte order mark.
fn utf32be_with_mark() -> Vec<u8> {
    let mut bytes = vec![0x00, 0x00, 0xFE, 0xFF];
    bytes.extend("<r/>".chars().flat_map(|c| u32::from(c).to_be_bytes()));
    bytes
}

#[test]
fn lib_diag_004_a_prose_value_is_neither_text_nor_a_count() {
    let value = prose("a quote");
    assert_eq!(
        (value.as_text(), value.as_count(), value.as_prose()),
        (None, None, Some("a quote")),
        "{ROW}: the accessors of Prose(\"a quote\")"
    );
    assert_eq!(
        (
            text("a quote").as_prose(),
            ArgumentValue::Count(1).as_prose()
        ),
        (None, None),
        "{ROW}: as_prose() of a Text or a Count is not None"
    );
    assert_eq!(
        value.to_string(),
        "a quote",
        "{ROW}: the Display of Prose does not write its text unchanged"
    );
    assert_eq!(
        prose("\"=\"").to_string(),
        "\"=\"",
        "{ROW}: the Display of Prose quotes or escapes its text"
    );
    assert_ne!(
        prose("x"),
        text("x"),
        "{ROW}: Prose(\"x\") equals Text(\"x\")"
    );
    assert_eq!(prose("x"), prose("x"), "{ROW}: Prose(\"x\") is not itself");

    // Text, then Count, then Prose, then by value.
    let mut values = vec![
        prose("b"),
        ArgumentValue::Count(2),
        text("z"),
        prose("a"),
        ArgumentValue::Count(1),
        text("a"),
    ];
    values.sort();
    assert_eq!(
        values,
        [
            text("a"),
            text("z"),
            ArgumentValue::Count(1),
            ArgumentValue::Count(2),
            prose("a"),
            prose("b"),
        ],
        "{ROW}: values do not order Text, then Count, then Prose, then by value"
    );
    assert_eq!(
        (
            text("z").cmp(&prose("a")),
            ArgumentValue::Count(u64::MAX).cmp(&prose("")),
        ),
        (Ordering::Less, Ordering::Less),
        "{ROW}: a Prose does not order after every Text and Count"
    );

    // A prose argument writes `name="text"`, Debug-quoted as a Text.
    let error = strict_refusal("<r a/>", b"<r a/>");
    let Some(expected) = error
        .arguments()
        .iter()
        .find(|argument| argument.name() == "expected")
    else {
        panic!("{ROW}: the refusal of <r a/> has no argument expected");
    };
    assert_eq!(
        expected.value(),
        &prose("\"=\""),
        "{ROW}: expected of <r a/> is not Prose"
    );
    assert_eq!(
        expected.to_string(),
        r#"expected="\"=\"""#,
        "{ROW}: a prose Argument does not write name=\"text\", the text Debug-quoted"
    );
}

/// A strict read that is refused: what it reads, the input, the template,
/// the arguments and the Display.
type Refused = (
    &'static str,
    Vec<u8>,
    &'static str,
    Vec<(&'static str, ArgumentValue)>,
    &'static str,
);

#[test]
fn lib_diag_004_each_prose_argument_a_refusal_reaches_is_prose() {
    let reads: [Refused; 5] = [
        (
            "a UTF-32 BE byte order mark",
            utf32be_with_mark(),
            "SACM-XML-002/byte-order-mark",
            vec![("encoding", prose("UTF-32 (big-endian)"))],
            "the reader cannot represent the input at line 1, column 1: the input starts with a \
             UTF-32 (big-endian) byte order mark, and only UTF-8, ASCII, ISO-8859-1 and UTF-16 \
             are read (a limit of this version)",
        ),
        (
            "UTF-16 LE without a byte order mark",
            utf16le("<?xml version=\"1.0\" encoding=\"UTF-16LE\"?><r/>"),
            "SACM-XML-002/detected-encoding",
            vec![("encoding", prose("UTF-16 (little-endian)"))],
            "the reader cannot represent the input at line 1, column 1: the input is encoded in \
             UTF-16 (little-endian), and only UTF-8, ASCII, ISO-8859-1 and UTF-16 starting with \
             U+FEFF are read (a limit of this version)",
        ),
        (
            "<?xml ?><r/>",
            b"<?xml ?><r/>".to_vec(),
            "SACM-XML-001/declaration-character",
            vec![("expected", prose("\"version\""))],
            "the input is not well-formed XML at line 1, column 7: an unexpected character in \
             the XML declaration where \"version\" was expected",
        ),
        (
            "<r a/>",
            b"<r a/>".to_vec(),
            "SACM-XML-001/unexpected-character",
            vec![("expected", prose("\"=\""))],
            "the input is not well-formed XML at line 1, column 5: an unexpected character where \
             \"=\" was expected",
        ),
        (
            "<r a=b/>",
            b"<r a=b/>".to_vec(),
            "SACM-XML-001/unexpected-character",
            vec![("expected", prose("a quote"))],
            "the input is not well-formed XML at line 1, column 6: an unexpected character where \
             a quote was expected",
        ),
    ];
    for (what, input, template, arguments, display) in reads {
        let error = strict_refusal(what, &input);
        assert_eq!(
            error.template(),
            template,
            "{ROW}: {what}: setup: another template"
        );
        assert_eq!(
            named(error.arguments()),
            arguments,
            "{ROW}: {what}: the arguments by name and variant"
        );
        assert_eq!(
            error.to_string(),
            display,
            "{ROW}: {what}: the Display is not 0.1.0's"
        );
    }

    let error = unrepresentable_refusal();
    assert_eq!(
        error.template(),
        "SACM-XML-003/unrepresentable-character",
        "{ROW}: U+0001 in a name: setup: another template"
    );
    assert_eq!(
        named(error.arguments()),
        [
            ("element", text("arguments")),
            ("attribute", prose("name's content")),
            ("code_point", ArgumentValue::Count(1)),
        ],
        "{ROW}: U+0001 in a name: the arguments by name and variant"
    );
    assert_eq!(
        error.to_string(),
        "the element \"arguments\" cannot be written: its name's content holds U+0001, which \
         XML 1.0 cannot represent",
        "{ROW}: U+0001 in a name: the Display is not 0.1.0's"
    );
}

#[cfg(feature = "serde")]
mod serde_records {
    use super::{ROW, WriteError, named, prose, text, unrepresentable_refusal};
    use sacm::diagnostic::ArgumentValue;

    /// The strict write refusal of a name holding U+0001, as
    /// `serde_refusals.rs` pins it: its `attribute` argument is prose.
    const UNREPRESENTABLE: &str = r#"{"type":"UnrepresentableCharacter","code":"SACM-XML-003","template":"SACM-XML-003/unrepresentable-character","arguments":{"element":"arguments","attribute":"name's content","code_point":1},"element":"arguments","character":"\u0001","attribute":"NameContent"}"#;

    #[test]
    fn lib_diag_004_a_prose_argument_is_a_json_string_and_reads_back_as_prose() {
        let refusal = unrepresentable_refusal();
        let written = match serde_json::to_string(&refusal) {
            Ok(json) => json,
            Err(error) => panic!("{ROW}: the refusal does not serialize: {error}"),
        };
        assert_eq!(
            written, UNREPRESENTABLE,
            "{ROW}: the JSON of a refusal with a prose argument is not unchanged"
        );
        let read: WriteError = match serde_json::from_str(UNREPRESENTABLE) {
            Ok(read) => read,
            Err(error) => panic!("{ROW}: the JSON does not read back: {error}"),
        };
        assert_eq!(
            named(read.arguments()),
            [
                ("element", text("arguments")),
                ("attribute", prose("name's content")),
                ("code_point", ArgumentValue::Count(1)),
            ],
            "{ROW}: a string for a prose argument does not read back as Prose"
        );
        assert_eq!(read, refusal, "{ROW}: the record does not read back equal");
        match serde_json::to_string(&read) {
            Ok(json) => assert_eq!(
                json, UNREPRESENTABLE,
                "{ROW}: the record read back does not write the same JSON"
            ),
            Err(error) => panic!("{ROW}: the record read back does not serialize: {error}"),
        }

        let number =
            UNREPRESENTABLE.replacen(r#""attribute":"name's content""#, r#""attribute":1"#, 1);
        assert_ne!(number, UNREPRESENTABLE, "{ROW}: setup: no prose attribute");
        match serde_json::from_str::<WriteError>(&number) {
            Ok(value) => panic!("{ROW}: a number for a prose argument reads as {value:?}"),
            Err(error) => {
                assert!(
                    error.is_data(),
                    "{ROW}: a number for a prose argument is refused as malformed JSON: {error}"
                );
                assert!(
                    error
                        .to_string()
                        .contains("the argument \"attribute\" is not of the kind Prose"),
                    "{ROW}: the refusal of a number for a prose argument does not name its \
                     kind: {error}"
                );
            }
        }
    }
}
