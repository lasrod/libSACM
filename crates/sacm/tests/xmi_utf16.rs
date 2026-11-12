//! Tests for UTF-16 input to `sacm::xmi::read` (SACM23-XMI-018; XML 1.0
//! (Fifth Edition) §2.2, §4.3.3; ADR 0005 B2 amendment 1).
//!
//! The minimal UTF-16 support: input that starts with a UTF-16 byte order
//! mark is decoded and then read exactly as the same characters in UTF-8.
//! Its declaration may be absent or name `UTF-16`, `UTF-16LE` or `UTF-16BE`.
//! Another declared name, an odd byte count and an unpaired surrogate are
//! `Malformed`. Everything else stays `Unsupported` (LIB-API-009).
//!
//! Every input is written here as UTF-8 text and encoded by a helper, so
//! the UTF-16 form and its UTF-8 reference hold the same characters. No
//! fixture file is used: the encodings are the point, and a file would hold
//! only one of them. `read` is called with `ReadOptions::default()`.
//!
//! Not checked here:
//! - Item 10 (the W3C suite): `w3c_xml_conformance.rs`.
//! - Item 11 (docs): checked by the verifier.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, Diagnostic, SourceLocation};
use sacm::model::{Document, ElementKind};
use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "SACM23-XMI-018";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// A readable bare root.
const AP_ROOT: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"arg_1\"/>",
);

/// A readable document after its declaration: two wrapped roots, with
/// non-ASCII characters (one outside the BMP) in comments and an id, and
/// line ends of each kind.
const BODY: &str = concat!(
    "<!-- café ☕ 😀 -->\r\n",
    "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n",
    "  <sacm:ArgumentPackage xmi:id=\"a\"/>\r",
    "  <!-- ✓ -->\n",
    "  <sacm:AssuranceCasePackage xmi:id=\"n_é\"/>\n",
    "</xmi:XMI>\n",
);

// --- Helpers --------------------------------------------------------------

/// How long one `read` may take. Reading any input here takes milliseconds;
/// a scan that stops advancing never finishes. Well below cargo-mutants'
/// minimum test timeout of 20 seconds, as in `xmi_read.rs`.
const DEADLINE: Duration = Duration::from_secs(5);

/// `xmi::read` with the default options, on a worker thread. Fails the test
/// when the read panics or does not return within [`DEADLINE`].
fn read(input: &[u8]) -> Result<ReadOutput, ReadError> {
    let owned = input.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default()));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{ROW}: read did not return within {DEADLINE:?} for {}",
            preview(input)
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("{ROW}: read panicked on {}", preview(input))
        }
    }
}

/// The first bytes of `input`, in hex, for assertion messages.
fn preview(input: &[u8]) -> String {
    let end = input.len().min(48);
    let hex: Vec<String> = input[..end].iter().map(|b| format!("{b:02X}")).collect();
    if end < input.len() {
        format!("[{}...] ({} bytes)", hex.join(" "), input.len())
    } else {
        format!("[{}]", hex.join(" "))
    }
}

/// A 1-based location that the test expects `SourceLocation::new` to accept.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// The two byte orders, with a name for messages.
const ORDERS: [(&str, Order); 2] = [("UTF-16LE", Order::Little), ("UTF-16BE", Order::Big)];

#[derive(Debug, Clone, Copy)]
enum Order {
    Little,
    Big,
}

/// `units`, as bytes in `order`, after the byte order mark U+FEFF.
fn units_with_bom(units: &[u16], order: Order) -> Vec<u8> {
    std::iter::once(0xFEFF_u16)
        .chain(units.iter().copied())
        .flat_map(|unit| match order {
            Order::Little => unit.to_le_bytes(),
            Order::Big => unit.to_be_bytes(),
        })
        .collect()
}

/// `text` encoded as UTF-16 in `order`, with a byte order mark.
fn utf16(text: &str, order: Order) -> Vec<u8> {
    units_with_bom(&text.encode_utf16().collect::<Vec<_>>(), order)
}

/// `text` encoded as UTF-16 in `order`, without a byte order mark.
fn utf16_without_bom(text: &str, order: Order) -> Vec<u8> {
    let mut bytes = utf16(text, order);
    bytes.drain(..2);
    bytes
}

/// `text` encoded as UCS-4, each character's big-endian bytes in `order`:
/// `[0, 1, 2, 3]` is big-endian (1234), `[3, 2, 1, 0]` little-endian (4321).
fn ucs4(text: &str, order: [usize; 4]) -> Vec<u8> {
    text.chars()
        .flat_map(|c| {
            let big = u32::from(c).to_be_bytes();
            order.map(|index| big[index])
        })
        .collect()
}

/// What two reads must share to be "the same result": for `Ok`, the roots
/// (id, kind, order), every element's id, kind and parent, and the
/// diagnostics; for `Err`, the whole `ReadError` (variant, location, reason).
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Read {
        roots: Vec<(String, ElementKind)>,
        elements: Vec<(String, ElementKind, Option<String>)>,
        diagnostics: Vec<Diagnostic>,
    },
    Refused(ReadError),
}

fn outcome(input: &[u8]) -> Outcome {
    match read(input) {
        Ok(output) => {
            let document: &Document = output.document();
            Outcome::Read {
                roots: document
                    .roots()
                    .map(|root| (root.id().as_str().to_owned(), root.kind()))
                    .collect(),
                elements: document
                    .elements()
                    .map(|element| {
                        (
                            element.id().as_str().to_owned(),
                            element.kind(),
                            element.parent().map(|p| p.id().as_str().to_owned()),
                        )
                    })
                    .collect(),
                diagnostics: output.diagnostics().to_vec(),
            }
        }
        Err(error) => Outcome::Refused(error),
    }
}

/// Asserts that `text` as UTF-16 with a byte order mark, in both byte
/// orders, gives the same outcome as `text` in UTF-8, and returns it.
fn assert_same_as_utf8(what: &str, text: &str) -> Outcome {
    let expected = outcome(text.as_bytes());
    for (name, order) in ORDERS {
        let input = utf16(text, order);
        assert_eq!(
            outcome(&input),
            expected,
            "{ROW}: {what}: {text:?} as {name} with a byte order mark does not read as in UTF-8"
        );
    }
    expected
}

/// Asserts that `input` is refused as `Malformed` (SACM-XML-001) at
/// `location`, and returns the error.
fn assert_malformed_at(what: &str, input: &[u8], location: SourceLocation) -> ReadError {
    let result = read(input);
    let Err(error @ ReadError::Malformed { .. }) = &result else {
        panic!(
            "{ROW}: {what}: expected Malformed for {}, got {result:?}",
            preview(input)
        );
    };
    assert_eq!(
        error.code(),
        Some(Code::XML_MALFORMED),
        "{ROW}: {what}: the Malformed refusal does not carry SACM-XML-001"
    );
    assert_eq!(
        error.location(),
        Some(location),
        "{ROW}: {what}: the Malformed refusal of {} is not at {location}: {error}",
        preview(input)
    );
    error.clone()
}

/// Asserts that `input` is refused as `Unsupported`, with no code, at
/// `location`.
fn assert_unsupported_at(what: &str, input: &[u8], location: SourceLocation) {
    let result = read(input);
    let Err(error @ ReadError::Unsupported { .. }) = &result else {
        panic!(
            "{ROW}: {what}: expected Unsupported for {}, got {result:?}",
            preview(input)
        );
    };
    assert_eq!(
        error.code(),
        None,
        "{ROW}: {what}: the Unsupported refusal carries a code"
    );
    assert_eq!(
        error.location(),
        Some(location),
        "{ROW}: {what}: the Unsupported refusal of {} is not at {location}: {error}",
        preview(input)
    );
}

// --- Item 1. Reading with a byte order mark -------------------------------------

#[test]
fn sacm23_xmi_018_utf16_with_a_bom_reads_as_the_utf8_document() {
    let expected = Outcome::Read {
        roots: vec![
            ("a".to_owned(), ElementKind::ArgumentPackage),
            ("n_é".to_owned(), ElementKind::AssuranceCasePackage),
        ],
        elements: vec![
            ("a".to_owned(), ElementKind::ArgumentPackage, None),
            ("n_é".to_owned(), ElementKind::AssuranceCasePackage, None),
        ],
        diagnostics: Vec::new(),
    };
    // The UTF-8 reference: the same characters, with no declaration and with
    // a UTF-8 one.
    for reference in [
        BODY.to_owned(),
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{BODY}"),
    ] {
        assert_eq!(
            outcome(reference.as_bytes()),
            expected,
            "{ROW}: the UTF-8 reference document does not read as expected"
        );
    }
    let declarations = [
        ("no declaration", String::new()),
        (
            "a declaration without an encoding",
            "<?xml version=\"1.0\"?>\n".to_owned(),
        ),
        (
            "encoding=\"UTF-16\"",
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n".to_owned(),
        ),
        (
            "encoding=\"utf-16\"",
            "<?xml version=\"1.0\" encoding=\"utf-16\"?>\n".to_owned(),
        ),
        (
            "encoding=\"UTF-16LE\"",
            "<?xml version=\"1.0\" encoding=\"UTF-16LE\"?>\n".to_owned(),
        ),
        (
            "encoding='UTF-16BE' and standalone",
            "<?xml version='1.0' encoding='UTF-16BE' standalone='yes'?>\n".to_owned(),
        ),
    ];
    for (what, declaration) in declarations {
        let text = format!("{declaration}{BODY}");
        for (name, order) in ORDERS {
            assert_eq!(
                outcome(&utf16(&text, order)),
                expected,
                "{ROW}: {name} with a byte order mark and {what} does not read as the UTF-8 \
                 document"
            );
        }
    }
}

// --- Item 2. A declared name that contradicts the mark ------------------------

#[test]
fn sacm23_xmi_018_a_non_utf16_name_after_a_utf16_bom_is_malformed_at_the_name() {
    let declaration = |name: &str| format!("<?xml version=\"1.0\" encoding=\"{name}\"?>");
    for name in ["UTF-8", "US-ASCII", "ISO-8859-1"] {
        let text = format!("{}\n{AP_ROOT}", declaration(name));
        for (order_name, order) in ORDERS {
            let plain = utf16(&text, order);
            // The declaration is checked before the decoding errors after
            // it: a lone surrogate after the declaration, or an odd byte
            // count, does not hide the contradiction.
            let mut units: Vec<u16> = declaration(name).encode_utf16().collect();
            units.push(0xD800);
            units.extend(format!("\n{AP_ROOT}").encode_utf16());
            let surrogate = units_with_bom(&units, order);
            let mut odd = plain.clone();
            odd.push(0x0A);
            let inputs = [
                ("", plain),
                (", then a lone D800", surrogate),
                (", plus one trailing byte", odd),
            ];
            for (variant, input) in inputs {
                let what =
                    format!("{order_name} with a byte order mark declaring {name:?}{variant}");
                let error = assert_malformed_at(&what, &input, loc(1, 31));
                let shown = error.to_string();
                assert!(
                    shown.contains(&format!("{name:?}")) && shown.contains("contradicts"),
                    "{ROW}: {what}: the refusal does not name the contradicting {name:?}: {shown}"
                );
            }
        }
    }
    // The exact Display, in both byte orders.
    let text = format!("{}\n{AP_ROOT}", declaration("UTF-8"));
    for (order_name, order) in ORDERS {
        let result = read(&utf16(&text, order));
        let shown = result.as_ref().map_err(ToString::to_string).err();
        assert_eq!(
            shown.as_deref(),
            Some(
                "the input is not well-formed XML at line 1, column 31: the declared encoding \
                 \"UTF-8\" contradicts the UTF-16 byte order mark"
            ),
            "{ROW}: {order_name} declaring \"UTF-8\": unexpected Display ({result:?})"
        );
    }
}

// --- Item 3. An odd byte count ----------------------------------------------------

#[test]
fn sacm23_xmi_018_an_odd_byte_count_is_malformed_after_the_last_character() {
    // A complete document: its ten characters on line 2 end before column
    // 11 (U+1F600 is one column), or, after a final line feed, at 3:1.
    let documents = [
        ("<?xml version=\"1.0\"?>\n<r>😀 é</r>", loc(2, 11)),
        ("<?xml version=\"1.0\"?>\r\n<r/>\n", loc(3, 1)),
    ];
    for (text, end) in documents {
        for (name, order) in ORDERS {
            for trailing in [0x00, 0x0A, 0x3C] {
                let mut input = utf16(text, order);
                input.push(trailing);
                assert_malformed_at(
                    &format!("{text:?} as {name} plus the byte {trailing:02X}"),
                    &input,
                    end,
                );
            }
        }
    }
    assert_malformed_at("FF FE 3C", &[0xFF, 0xFE, 0x3C], loc(1, 1));
    assert_malformed_at("FE FF 00", &[0xFE, 0xFF, 0x00], loc(1, 1));
}

// --- Item 4. Unpaired surrogates ---------------------------------------------------

#[test]
fn sacm23_xmi_018_an_unpaired_surrogate_is_malformed_at_the_surrogate() {
    let text = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    let join = |parts: &[&[u16]]| parts.concat();
    let open = text("<r>");
    let close = text("</r>");
    let cases: [(&str, Vec<u16>, SourceLocation); 6] = [
        (
            "a lone D800 in text",
            join(&[&open, &[0xD800], &close]),
            loc(1, 4),
        ),
        (
            "a lone DC00 in text",
            join(&[&open, &[0xDC00], &close]),
            loc(1, 4),
        ),
        (
            "a high surrogate as the last code unit",
            join(&[&open, &[0xD800]]),
            loc(1, 4),
        ),
        (
            "a high surrogate before a code unit that is not a low surrogate",
            join(&[&open, &[0xD800, 0x0041], &close]),
            loc(1, 4),
        ),
        (
            "a lone D800 after a valid pair, one column",
            join(&[&open, &text("😀"), &[0xD800], &close]),
            loc(1, 5),
        ),
        (
            "a lone DBFF on line 2 after a CRLF",
            join(&[&open, &text("\r\n"), &[0xDBFF], &close]),
            loc(2, 1),
        ),
    ];
    for (what, units, location) in cases {
        for (name, order) in ORDERS {
            assert_malformed_at(
                &format!("{what}, {name}"),
                &units_with_bom(&units, order),
                location,
            );
        }
    }
}

#[test]
fn sacm23_xmi_018_a_surrogate_pair_in_an_id_reads_as_in_utf8() {
    let text = format!("<sacm:ArgumentPackage{NS} xmi:id=\"a😀\"/>");
    let expected = Outcome::Read {
        roots: vec![("a😀".to_owned(), ElementKind::ArgumentPackage)],
        elements: vec![("a😀".to_owned(), ElementKind::ArgumentPackage, None)],
        diagnostics: Vec::new(),
    };
    assert_eq!(
        assert_same_as_utf8("a U+1F600 id", &text),
        expected,
        "{ROW}: the id with U+1F600 is not read exactly"
    );
}

// --- Item 5. A byte order mark and nothing else ------------------------------------

#[test]
fn sacm23_xmi_018_a_bom_with_no_root_is_malformed_as_in_utf8() {
    // The UTF-8 reference (LIB-SEC-002): with no root element, the refusal
    // is at the end of the input, which is 1:1 for the mark alone; the mark
    // takes no column.
    for (what, white_space, end) in [
        ("alone", "", loc(1, 1)),
        ("before white space", " \t", loc(1, 3)),
        ("before line ends", "\r\n \n", loc(3, 1)),
    ] {
        let utf8_text = format!("\u{FEFF}{white_space}");
        assert_malformed_at(
            &format!("the UTF-8 byte order mark {what}"),
            utf8_text.as_bytes(),
            end,
        );
        let utf8_result = outcome(utf8_text.as_bytes());
        for (name, order) in ORDERS {
            let input = utf16(white_space, order);
            assert_malformed_at(&format!("a {name} byte order mark {what}"), &input, end);
            assert_eq!(
                outcome(&input),
                utf8_result,
                "{ROW}: a {name} byte order mark {what} is not refused as the UTF-8 one is"
            );
        }
    }
}

// --- Item 6. Locations --------------------------------------------------------------

#[test]
fn sacm23_xmi_018_locations_count_characters_and_the_bom_takes_no_column() {
    let cases = [
        ("<!--😀--><r/>", loc(1, 9)),
        ("<!--😀-->\n<r/>", loc(2, 1)),
        ("<!--😀-->\r\n<r/>", loc(2, 1)),
        ("<!--😀-->\r<r/>", loc(2, 1)),
    ];
    for (text, location) in cases {
        let utf8 = read(text.as_bytes());
        assert!(
            matches!(&utf8, Err(ReadError::InvalidRoot { location: at, .. }) if *at == Some(location)),
            "{ROW}: {text:?} in UTF-8 is not InvalidRoot at {location}: {utf8:?}"
        );
        for (name, order) in ORDERS {
            assert_eq!(
                outcome(&utf16(text, order)),
                outcome(text.as_bytes()),
                "{ROW}: {text:?} as {name} with a byte order mark is not refused as in UTF-8, \
                 InvalidRoot at {location}"
            );
        }
    }
}

// --- Item 7. The same output as UTF-8 ----------------------------------------------

#[test]
fn sacm23_xmi_018_inputs_from_the_read_and_root_tests_give_the_utf8_result() {
    let wrapper = |content: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">{content}</xmi:XMI>"
        )
    };
    // Taken from the xmi_roots.rs and xmi_read.rs tests, without encoding
    // declarations: readable roots, root-stage refusals, and refusals of the
    // input layer.
    let inputs: Vec<(&str, String)> = vec![
        (
            "an AssuranceCasePackage root",
            format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"acp_1\"/>"),
        ),
        (
            "three wrapped roots",
            wrapper(
                "<sacm:AssuranceCasePackage xmi:id=\"n1\"/><sacm:ArgumentPackage xmi:id=\"a1\"/>\
                 <sacm:AssuranceCasePackage xmi:id=\"n2\"/>",
            ),
        ),
        (
            "a declaration, comments, PIs and white space",
            format!(
                "<?xml version=\"1.0\"?>\n<!-- c -->\n<?tool a?>\n\
                 <sacm:ArgumentPackage{NS} xmi:id=\"a\" xmi:version=\"2.0\" xmlns:foo=\"urn:x\">\n  \
                 <!-- c --><?tool b?>\n</sacm:ArgumentPackage>\n<!-- c -->\n"
            ),
        ),
        (
            "a redundant xmi:type",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\" xmi:type=\"sacm:ArgumentPackage\"/>"),
        ),
        ("a Claim root", format!("<sacm:Claim{NS} xmi:id=\"c1\"/>")),
        (
            "a wrapper with no package, on line 2",
            format!("<!-- c -->\n{}", wrapper("")),
        ),
        (
            "an ArtifactPackage root",
            format!("<sacm:ArtifactPackage{NS} xmi:id=\"artifacts_1\"/>"),
        ),
        (
            "an unread attribute",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\" foo=\"bar\"/>"),
        ),
        (
            "a repeated root id",
            wrapper(
                "\n  <sacm:ArgumentPackage xmi:id=\"p\"/>\n  <sacm:ArgumentPackage xmi:id=\"p\"/>\n",
            ),
        ),
        (
            "a DOCTYPE after a declaration",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE r>\n<r/>".to_owned(),
        ),
        (
            "a mismatched end tag",
            "<?xml version=\"1.0\"?>\n<r></s>".to_owned(),
        ),
        ("a truncated start tag", "<r".to_owned()),
        (
            "a character reference to a surrogate",
            "<r>&#xD800;</r>".to_owned(),
        ),
        (
            "a declared xmlns prefix",
            format!("<sacm:ArgumentPackage{NS} xmlns:xmlns=\"urn:x\" xmi:id=\"a\"/>"),
        ),
        (
            "a malformed declaration",
            "<?xml version=\"2.0\"?><r/>".to_owned(),
        ),
    ];
    // The list holds documents and refusals of each variant, so the
    // comparison covers both arms.
    let mut kinds = std::collections::BTreeSet::new();
    for (_, text) in &inputs {
        kinds.insert(match outcome(text.as_bytes()) {
            Outcome::Read { .. } => "Ok",
            Outcome::Refused(ReadError::Doctype { .. }) => "Doctype",
            Outcome::Refused(ReadError::Malformed { .. }) => "Malformed",
            Outcome::Refused(ReadError::InvalidRoot { .. }) => "InvalidRoot",
            Outcome::Refused(ReadError::NoPackage { .. }) => "NoPackage",
            Outcome::Refused(ReadError::Unsupported { .. }) => "Unsupported",
            Outcome::Refused(_) => "another refusal",
        });
    }
    assert_eq!(
        kinds.into_iter().collect::<Vec<_>>(),
        [
            "Doctype",
            "InvalidRoot",
            "Malformed",
            "NoPackage",
            "Ok",
            "Unsupported"
        ],
        "{ROW}: the UTF-8 inputs do not cover a document and each refusal"
    );
    for (what, text) in &inputs {
        assert_same_as_utf8(what, text);
    }
}

// --- Item 8. DOCTYPE ------------------------------------------------------------------

#[test]
fn sacm23_xmi_018_a_doctype_in_utf16_is_refused_at_its_start_and_not_expanded() {
    let cases = [
        (
            "an external entity after a UTF-16 declaration",
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n\
             <!DOCTYPE r [ <!ENTITY xxe SYSTEM \"file:///etc/passwd\"> ]>\n<r>&xxe;</r>",
            loc(2, 1),
        ),
        (
            "an external entity first, after a comment with U+1F600",
            "<!--😀--><!DOCTYPE r SYSTEM \"http://example.invalid/r.dtd\"><r/>",
            loc(1, 9),
        ),
        (
            "an internal entity with a marker value",
            "<!DOCTYPE r [ <!ENTITY m \"EXPANDED-MARKER\"> ]><r>&m;</r>",
            loc(1, 1),
        ),
    ];
    for (what, text, location) in cases {
        for (name, order) in ORDERS {
            let input = utf16(text, order);
            let result = read(&input);
            let Err(error @ ReadError::Doctype { .. }) = &result else {
                panic!("{ROW}: {what}, {name}: expected Doctype, got {result:?}");
            };
            assert_eq!(
                error.code(),
                Some(Code::XML_DOCTYPE_REJECTED),
                "{ROW}: {what}, {name}: the Doctype refusal does not carry SACM-SEC-001"
            );
            assert_eq!(
                error.location(),
                Some(location),
                "{ROW}: {what}, {name}: the Doctype refusal is not at the `<` of `<!DOCTYPE`"
            );
            let shown = format!("{error} {error:?}");
            for expansion in ["EXPANDED-MARKER", "root:"] {
                assert!(
                    !shown.contains(expansion),
                    "{ROW}: {what}, {name}: the refusal shows expanded text: {shown}"
                );
            }
        }
    }
}

// --- Item 9. What stays Unsupported ------------------------------------------------

#[test]
fn sacm23_xmi_018_utf16_without_a_bom_stays_unsupported_at_1_1() {
    for declaration in [
        "<?xml version=\"1.0\"?>",
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>",
        "<?xml version=\"1.0\" encoding=\"UTF-16LE\"?>",
        "<?xml version=\"1.0\" encoding=\"UTF-16BE\"?>",
    ] {
        let text = format!("{declaration}\n{AP_ROOT}");
        for (name, order) in ORDERS {
            let input = utf16_without_bom(&text, order);
            let first: &[u8] = match order {
                Order::Little => &[0x3C, 0x00, 0x3F, 0x00],
                Order::Big => &[0x00, 0x3C, 0x00, 0x3F],
            };
            assert_eq!(
                input.get(..4),
                Some(first),
                "{ROW}: the BOM-less {name} test input does not start with its Appendix F bytes"
            );
            assert_unsupported_at(
                &format!("BOM-less {name} with {declaration:?}"),
                &input,
                loc(1, 1),
            );
        }
    }
}

#[test]
fn sacm23_xmi_018_utf32_and_ucs4_stay_unsupported_at_1_1() {
    let text = format!("<?xml version=\"1.0\"?>\n{AP_ROOT}");
    let with_bom = format!("\u{FEFF}{text}");
    // Byte orders as indexes of the big-endian bytes (XML 1.0 Appendix F).
    let orders: [(&str, [usize; 4]); 4] = [
        ("1234 (UTF-32BE)", [0, 1, 2, 3]),
        ("4321 (UTF-32LE)", [3, 2, 1, 0]),
        ("2143", [1, 0, 3, 2]),
        ("3412", [2, 3, 0, 1]),
    ];
    for (name, order) in orders {
        assert_unsupported_at(
            &format!("UCS-4 {name} with a byte order mark"),
            &ucs4(&with_bom, order),
            loc(1, 1),
        );
        assert_unsupported_at(
            &format!("UCS-4 {name} without a byte order mark"),
            &ucs4(&text, order),
            loc(1, 1),
        );
    }
    // FF FE 00 00 is the UTF-32LE mark, not a UTF-16 mark and U+0000.
    for (what, input) in [
        ("FF FE 00 00 alone", &[0xFF, 0xFE, 0x00, 0x00][..]),
        ("00 00 FE FF alone", &[0x00, 0x00, 0xFE, 0xFF]),
        (
            "FF FE 00 00 then `<` in UTF-32LE",
            &[0xFF, 0xFE, 0x00, 0x00, 0x3C, 0x00, 0x00, 0x00],
        ),
    ] {
        assert_unsupported_at(what, input, loc(1, 1));
    }
}

#[test]
fn sacm23_xmi_018_a_utf16_name_declared_in_utf8_stays_unsupported_at_the_name() {
    for name in ["UTF-16", "utf-16", "UTF-16LE", "UTF-16BE"] {
        let text = format!("<?xml version=\"1.0\" encoding=\"{name}\"?>\n{AP_ROOT}");
        assert_unsupported_at(
            &format!("UTF-8 declaring {name:?}"),
            text.as_bytes(),
            loc(1, 31),
        );
        let with_utf8_bom = format!("\u{FEFF}{text}");
        assert_unsupported_at(
            &format!("UTF-8 with its byte order mark declaring {name:?}"),
            with_utf8_bom.as_bytes(),
            loc(1, 31),
        );
    }
}
