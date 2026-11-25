//! A kept type attribute writes back readable, complete XMI (LIB-COMPAT-019;
//! ADR 0033 amendment 2, ADR 0044 amendment 1; D-PRESERVE, D-ROOT-NAME,
//! D-RT).
//!
//! XML 1.0 (Fifth Edition) §3.1 WFC: Unique Att Spec: "An attribute name
//! MUST NOT appear more than once in the same start-tag or empty-element
//! tag." Namespaces in XML 1.0 (Third Edition) §5 NSC: Prefix Declared: "The
//! namespace prefix, unless it is `xml` or `xmlns`, MUST have been declared in
//! a namespace declaration attribute in either the start-tag of the element
//! where the prefix is used or in an ancestor element". XMI 2.5.1 §9.5.2 rule
//! 2g (p. 63): the value of the type attribute "is defined by the XML Schema
//! Part 1: Structures specification to be a `QName`". A tolerant `xmi::write`:
//!
//! - binds the prefix that a kept type attribute's value names (`type` in XMI
//!   2.5.1's or EMF's namespace, or `xsi:type`), in both modes, and renames
//!   `sacm` or `xmi` bound elsewhere, or a default namespace, to the holder's
//!   `nsN` in the value (T1);
//! - reads `id`, `type`, `uuid` and `label` given in both XMI namespaces from
//!   XMI 2.5.1's, keeps EMF's as written and writes each once (T2, T3);
//! - writes an element that keeps an EMF `type`, and none in XMI 2.5.1's,
//!   with its own `xmi:type` (T4);
//! - and `MoveElement` and `CopyElements` refuse to make a child of an
//!   element that keeps an `xmi:type` or `xsi:type`, which only a root can
//!   carry (`EditError::KeptRootType`, SACM-CMD-005; T5, T6).
//!
//! Every input is one line, so every read position is `(1, column)`. Each
//! write is compared with its golden text (LF line ends, a final LF), read
//! back tolerantly `==`, and written again byte-identical. The design brief
//! is `target/slice-0076-design.md`.

use sacm::diagnostic::{Argument, Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CopyElements, EditError, MoveElement, Operation};
use sacm::model::{Document, ElementId, ElementKind, SacmElement};
use sacm::xmi::{self, Limits, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "LIB-COMPAT-019";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The XMI namespace EMF writes (`ExtendedMetaData.XMI_URI`).
const EMF_NS: &str = "http://www.omg.org/XMI";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// The application's profile, whose classes the inputs name.
const P: &str = "urn:example:profile";

/// The first line of every write.
const DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

const ATTRIBUTE_KEPT: &str = "SACM-XMI-003/attribute-kept";
const ELEMENT_KEPT: &str = "SACM-XMI-003/element-kept";
const WRAPPER_ATTRIBUTE_KEPT: &str = "SACM-XMI-003/wrapper-attribute-kept";
const PREFIX_RENAMED: &str = "SACM-XMI-003/prefix-renamed";
const EMF_READ: &str = "SACM-XMI-015/emf-xmi-namespace-read";
const XSI_TYPE: &str = "SACM-XMI-010/xsi-type";

/// The `KeptRootType` sample of `edit-errors.json`, as compact JSON.
#[cfg(feature = "serde")]
const KEPT_ROOT_TYPE_JSON: &str = r#"{"type":"KeptRootType","code":"SACM-CMD-005","element":"a","attribute":"xmi:type","value":"p:Bundle"}"#;

/// `edit-errors.json`, whose last sample is [`KEPT_ROOT_TYPE_JSON`].
#[cfg(feature = "serde")]
const EDIT_ERRORS_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/golden/serde/edit-errors.json"
);

// --- Helpers ---------------------------------------------------------------------

/// `H`: the bindings of `sacm` and `xmi` to SACM 2.3 and XMI 2.5.1.
fn h() -> String {
    format!("xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\"")
}

/// `R2`: the start of a written root ArgumentPackage, up to its attributes.
fn r2() -> String {
    format!("<sacm:ArgumentPackage {}", h())
}

/// The start tag of a written root AssuranceCasePackage `case`.
fn case_start() -> String {
    format!("<sacm:AssuranceCasePackage {} xmi:id=\"case\">", h())
}

/// The start tag of a written `xmi:XMI`, with nothing kept on it.
fn wrapper_start() -> String {
    format!("<xmi:XMI {}>", h())
}

/// `lines`, each ended by LF.
fn lines(lines: &[&str]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// A written root ArgumentPackage whose start tag is `start` and that holds
/// the name `a`.
fn root_golden(start: &str) -> String {
    lines(&[
        DECLARATION,
        start,
        "  <name lang=\"en\" content=\"a\" />",
        "</sacm:ArgumentPackage>",
    ])
}

/// A written root ArgumentPackage that is one empty-element tag.
fn empty_root_golden(tag: &str) -> String {
    lines(&[DECLARATION, tag])
}

/// The written [`e_input`]: the root `a` and the child whose start tag is
/// `child`, holding the name `c`.
fn e_golden(child: &str) -> String {
    lines(&[
        DECLARATION,
        &format!("{} xmi:id=\"a\">", r2()),
        "  <name lang=\"en\" content=\"a\" />",
        child,
        "    <name lang=\"en\" content=\"c\" />",
        "  </argumentElement>",
        "</sacm:ArgumentPackage>",
    ])
}

/// The written [`ac_input`]: the case and the child package whose start tag
/// is `child`, holding the name `p`.
fn ac_golden(child: &str) -> String {
    lines(&[
        DECLARATION,
        &case_start(),
        "  <name lang=\"en\" content=\"c\" />",
        child,
        "    <name lang=\"en\" content=\"p\" />",
        "  </argumentPackage>",
        "</sacm:AssuranceCasePackage>",
    ])
}

/// `E(child)`: the root ArgumentPackage `a`, binding `emf`, `xsi` and `p`,
/// holding its name and `child` at column 282.
fn e_input(child: &str) -> String {
    format!(
        "<sacm:ArgumentPackage {} xmlns:emf=\"{EMF_NS}\" xmlns:xsi=\"{XSI_NS}\" \
         xmlns:p=\"{P}\" xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>{child}\
         </sacm:ArgumentPackage>",
        h()
    )
}

/// The argumentElement `c1` with `attributes`, the first at column 299 of
/// [`e_input`].
fn c1(attributes: &str) -> String {
    format!(
        "<argumentElement {attributes} xmi:id=\"c1\"><name lang=\"en\" content=\"c\"/>\
         </argumentElement>"
    )
}

/// `R(attributes)`: the root ArgumentPackage `a`, binding `emf` and `p`,
/// with `attributes` before its `xmi:id`, the first at column 188.
fn r_input(attributes: &str) -> String {
    format!(
        "<sacm:ArgumentPackage {} xmlns:emf=\"{EMF_NS}\" xmlns:p=\"{P}\" {attributes} \
         xmi:id=\"a\"><name lang=\"en\" content=\"a\"/></sacm:ArgumentPackage>",
        h()
    )
}

/// `AC(child)`: the root AssuranceCasePackage `case`, binding `emf` and `p`,
/// holding its name and `child`.
fn ac_input(child: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage {} xmlns:emf=\"{EMF_NS}\" xmlns:p=\"{P}\" \
         xmi:id=\"case\"><name lang=\"en\" content=\"c\"/>{child}</sacm:AssuranceCasePackage>",
        h()
    )
}

/// The argumentPackage `p1` with `attributes`, the first at column 253 of
/// [`ac_input`].
fn p1(attributes: &str) -> String {
    format!(
        "<argumentPackage {attributes} xmi:id=\"p1\"><name lang=\"en\" content=\"p\"/>\
         </argumentPackage>"
    )
}

/// `M1`: in `xmi:XMI`, the AssuranceCasePackage `case`; the ArgumentPackage
/// roots `a` (`xmi:type="p:Bundle"`), `s` (`xsi:type="p:Bundle"`), `e`
/// (`emf:type="p:Bundle"`), `i` (`xmi:type="sacm:ArgumentPackageInterface"`),
/// `x` and `y`; and the `ArgumentPackageBinding` `b` (`xmi:type="p:X"`) of `x`
/// and `y`.
fn m1() -> String {
    format!(
        "<xmi:XMI {} xmlns:p=\"{P}\" xmlns:emf=\"{EMF_NS}\" xmlns:xsi=\"{XSI_NS}\">\
         <sacm:AssuranceCasePackage xmi:id=\"case\"><name lang=\"en\" content=\"c\"/>\
         </sacm:AssuranceCasePackage>\
         <sacm:ArgumentPackage xmi:type=\"p:Bundle\" xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>\
         </sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xsi:type=\"p:Bundle\" xmi:id=\"s\"><name lang=\"en\" content=\"s\"/>\
         </sacm:ArgumentPackage>\
         <sacm:ArgumentPackage emf:type=\"p:Bundle\" xmi:id=\"e\"><name lang=\"en\" content=\"e\"/>\
         </sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\">\
         <name lang=\"en\" content=\"i\"/></sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:id=\"x\"><name lang=\"en\" content=\"x\"/></sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:id=\"y\"><name lang=\"en\" content=\"y\"/></sacm:ArgumentPackage>\
         <sacm:ArgumentPackageBinding xmi:type=\"p:X\" xmi:id=\"b\" participantPackage=\"x y\">\
         <name lang=\"en\" content=\"b\"/></sacm:ArgumentPackageBinding></xmi:XMI>",
        h()
    )
}

fn tolerant() -> ReadOptions {
    ReadOptions::default().with_mode(Mode::Tolerant)
}

fn loc(line: u32, column: u32) -> SourceLocation {
    SourceLocation::new(line, column)
        .unwrap_or_else(|| panic!("{ROW}: setup: ({line}, {column}) is no location"))
}

/// The read of `input` with `options`, which must succeed.
fn read_with(what: &str, input: &str, options: &ReadOptions) -> ReadOutput {
    match xmi::read(input.as_bytes(), options) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: the read refuses it: {error:?} ({error})"),
    }
}

/// The tolerant write of `document`, which must succeed.
fn tolerant_write(what: &str, document: &Document) -> String {
    xmi::write(document, &WriteOptions::default().with_mode(Mode::Tolerant)).unwrap_or_else(
        |error| panic!("{ROW}: {what}: a tolerant write refuses it: {error:?} ({error})"),
    )
}

/// Asserts that the tolerant write of `document` reads back with `options`
/// `==` to it and writes again byte-identical; gives the text written.
fn assert_writes_back(what: &str, document: &Document, options: &ReadOptions) -> String {
    let text = tolerant_write(what, document);
    let back = match xmi::read(text.as_bytes(), options) {
        Ok(output) => output.into_document(),
        Err(error) => panic!(
            "{ROW}: {what}: the tolerant write does not read back: {error:?} ({error})\n{text}"
        ),
    };
    assert!(
        back == *document,
        "{ROW}: {what}: the tolerant write does not read back ==:\n{text}"
    );
    let again = tolerant_write(&format!("{what}, read back"), &back);
    assert_eq!(
        again, text,
        "{ROW}: {what}: the write of the document read back is not byte-identical"
    );
    text
}

/// Asserts that the tolerant write of `document` is `golden`, reads back
/// with `options` `==` and writes again byte-identical.
fn assert_writes_golden(what: &str, document: &Document, options: &ReadOptions, golden: &str) {
    let text = tolerant_write(what, document);
    assert_eq!(
        text, golden,
        "{ROW}: {what}: the tolerant write is not its golden text"
    );
    assert_writes_back(what, document, options);
}

/// Asserts that the tolerant write of `document` holds `line`, reads back
/// with `options` `==` and writes again byte-identical; gives the text.
fn assert_writes_containing(
    what: &str,
    document: &Document,
    options: &ReadOptions,
    line: &str,
) -> String {
    let text = tolerant_write(what, document);
    assert!(
        text.contains(line),
        "{ROW}: {what}: the tolerant write does not hold {line:?}:\n{text}"
    );
    assert_writes_back(what, document, options)
}

/// A diagnostic as the tests compare it: code, severity, template,
/// location, affected ids and arguments as `(name, text)`.
#[derive(Debug, PartialEq, Eq)]
struct Seen {
    code: String,
    severity: Severity,
    template: String,
    at: Option<SourceLocation>,
    affected: Vec<String>,
    arguments: Vec<(String, String)>,
}

/// The arguments as `(name, text)`; a Count shows as its number.
fn arguments_of(found: &[Argument]) -> Vec<(String, String)> {
    found
        .iter()
        .map(|argument| {
            let value = argument.value();
            let text = value.as_text().map_or_else(
                || {
                    value
                        .as_count()
                        .map_or_else(String::new, |count| count.to_string())
                },
                str::to_owned,
            );
            (argument.name().to_owned(), text)
        })
        .collect()
}

/// `found` as [`Seen`], in order.
fn seen(found: &[Diagnostic]) -> Vec<Seen> {
    found
        .iter()
        .map(|diagnostic| Seen {
            code: diagnostic.code().as_str().to_owned(),
            severity: diagnostic.severity(),
            template: diagnostic.template().to_owned(),
            at: diagnostic.location(),
            affected: diagnostic
                .affected()
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
            arguments: arguments_of(diagnostic.arguments()),
        })
        .collect()
}

fn expected(
    severity: Severity,
    template: &str,
    at: SourceLocation,
    affected: &[&str],
    arguments: &[(&str, &str)],
) -> Seen {
    let code = template.split('/').next().unwrap_or(template);
    Seen {
        code: code.to_owned(),
        severity,
        template: template.to_owned(),
        at: Some(at),
        affected: affected.iter().map(|id| (*id).to_owned()).collect(),
        arguments: arguments
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

/// An `attribute-kept` Warning of `attribute` at (1, `column`), held by the
/// `class` `id`.
fn kept(column: u32, attribute: &str, class: &str, id: &str) -> Seen {
    kept_with(Severity::Warning, loc(1, column), attribute, class, id)
}

fn kept_with(
    severity: Severity,
    at: SourceLocation,
    attribute: &str,
    class: &str,
    id: &str,
) -> Seen {
    expected(
        severity,
        ATTRIBUTE_KEPT,
        at,
        &[id],
        &[
            ("attribute", attribute),
            ("holder_class", class),
            ("holder_id", id),
        ],
    )
}

/// The SACM-XMI-015 Warning of the first EMF name, `name`, at (1, `column`).
fn emf_read(column: u32, name: &str) -> Seen {
    expected(
        Severity::Warning,
        EMF_READ,
        loc(1, column),
        &[],
        &[("name", name)],
    )
}

/// The `element-kept` Warning of an `argumentElement` at (1, `column`) in
/// the ArgumentPackage `a`.
fn element_kept(column: u32) -> Seen {
    expected(
        Severity::Warning,
        ELEMENT_KEPT,
        loc(1, column),
        &["a"],
        &[
            ("element", "argumentElement"),
            ("holder_class", "ArgumentPackage"),
            ("holder_id", "a"),
        ],
    )
}

/// Asserts that the read `output` reports exactly `wanted`, in order.
fn assert_found(what: &str, output: &ReadOutput, wanted: &[Seen]) {
    assert_eq!(
        seen(output.diagnostics()),
        wanted,
        "{ROW}: {what}: the diagnostics of the read, in order"
    );
}

/// The ids of the elements of `document`, in order.
fn ids(document: &Document) -> Vec<String> {
    document
        .elements()
        .map(|element| element.id().as_str().to_owned())
        .collect()
}

/// The kind of the element `id` of `document`, if any.
fn kind_of(document: &Document, id: &str) -> Option<ElementKind> {
    document.get(id).map(SacmElement::kind)
}

/// One read-and-write case: the input, the diagnostics of its tolerant
/// read, and its golden write.
struct WriteCase {
    what: String,
    input: String,
    found: Vec<Seen>,
    golden: String,
}

/// Reads `case` tolerantly and asserts its diagnostics and its write cycle;
/// gives the document.
fn assert_case(case: &WriteCase) -> Document {
    let output = read_with(&case.what, &case.input, &tolerant());
    assert_found(&case.what, &output, &case.found);
    let document = output.into_document();
    assert_writes_golden(&case.what, &document, &tolerant(), &case.golden);
    document
}

// --- T1. A kept type attribute's value keeps its prefix binding -------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one table of the row's cases a to k, each with its input, diagnostics and golden"
)]
fn lib_compat_019_a_kept_type_attribute_keeps_the_binding_of_its_value_prefix() {
    let h = h();
    let r2 = r2();
    let a_input = format!(
        "<sacm:ArgumentPackage {h} xmlns:p=\"{P}\" xmi:type=\"p:Bundle\" xmi:id=\"a\">\
         <name lang=\"en\" content=\"a\"/></sacm:ArgumentPackage>"
    );
    let a_golden = root_golden(&format!(
        "{r2} xmi:id=\"a\" xmlns:p=\"{P}\" xmi:type=\"p:Bundle\">"
    ));
    let mut cases = vec![
        WriteCase {
            what: "(a) a root's xmi:type naming a profile class".to_owned(),
            input: a_input.clone(),
            found: vec![kept(153, "xmi:type", "ArgumentPackage", "a")],
            golden: a_golden.clone(),
        },
        WriteCase {
            what: "(b) a root's xsi:type naming a profile class".to_owned(),
            input: format!(
                "<sacm:ArgumentPackage {h} xmlns:p=\"{P}\" xmlns:xsi=\"{XSI_NS}\" \
                 xsi:type=\"p:Bundle\" xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>\
                 </sacm:ArgumentPackage>"
            ),
            found: vec![kept(207, "xsi:type", "ArgumentPackage", "a")],
            golden: root_golden(&format!(
                "{r2} xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"a\" xmlns:p=\"{P}\" \
                 xmlns:xsi=\"{XSI_NS}\" xsi:type=\"p:Bundle\">"
            )),
        },
        WriteCase {
            what: "(c) the value's prefix bound on xmi:XMI".to_owned(),
            input: format!(
                "<xmi:XMI {h} xmlns:p=\"{P}\"><sacm:ArgumentPackage xmi:type=\"p:Bundle\" \
                 xmi:id=\"a\"><name lang=\"en\" content=\"a\"/></sacm:ArgumentPackage>\
                 <sacm:ArgumentPackage xmi:id=\"b\"><name lang=\"en\" content=\"b\"/>\
                 </sacm:ArgumentPackage></xmi:XMI>"
            ),
            found: vec![kept(162, "xmi:type", "ArgumentPackage", "a")],
            golden: lines(&[
                DECLARATION,
                &wrapper_start(),
                &format!(
                    "  <sacm:ArgumentPackage xmi:id=\"a\" xmlns:p=\"{P}\" xmi:type=\"p:Bundle\">"
                ),
                "    <name lang=\"en\" content=\"a\" />",
                "  </sacm:ArgumentPackage>",
                "  <sacm:ArgumentPackage xmi:id=\"b\">",
                "    <name lang=\"en\" content=\"b\" />",
                "  </sacm:ArgumentPackage>",
                "</xmi:XMI>",
            ]),
        },
        WriteCase {
            what: "(d) the value's prefix sacm bound to another namespace".to_owned(),
            input: format!(
                "<s:ArgumentPackage xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:other\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:type=\"sacm:Bundle\" xmi:id=\"a\">\
                 <name lang=\"en\" content=\"a\"/></s:ArgumentPackage>"
            ),
            found: vec![kept(140, "xmi:type", "ArgumentPackage", "a")],
            golden: root_golden(&format!(
                "{r2} xmi:id=\"a\" xmlns:ns1=\"urn:other\" xmi:type=\"ns1:Bundle\">"
            )),
        },
        WriteCase {
            what: "(e) an unprefixed value under a default namespace".to_owned(),
            input: format!(
                "<sacm:ArgumentPackage {h} xmlns=\"urn:d\" xmi:type=\"Bundle\" xmi:id=\"a\"/>"
            ),
            found: vec![kept(137, "xmi:type", "ArgumentPackage", "a")],
            golden: empty_root_golden(&format!(
                "{r2} xmi:id=\"a\" xmlns:ns1=\"urn:d\" xmi:type=\"ns1:Bundle\" />"
            )),
        },
        WriteCase {
            what: "(f) a value with white space around it".to_owned(),
            input: a_input.replace("xmi:type=\"p:Bundle\"", "xmi:type=\" p:Bundle \""),
            found: vec![kept(153, "xmi:type", "ArgumentPackage", "a")],
            golden: root_golden(&format!(
                "{r2} xmi:id=\"a\" xmlns:p=\"{P}\" xmi:type=\" p:Bundle \">"
            )),
        },
    ];
    for value in [
        "sacm:Claim",
        "xmi:Bundle",
        "q:Bundle",
        "xml:Bundle",
        "xmlns:Bundle",
    ] {
        cases.push(WriteCase {
            what: format!("(g) the value {value:?}, which adds no binding"),
            input: format!("<sacm:ArgumentPackage {h} xmi:type=\"{value}\" xmi:id=\"a\"/>"),
            found: vec![kept(123, "xmi:type", "ArgumentPackage", "a")],
            golden: empty_root_golden(&format!("{r2} xmi:id=\"a\" xmi:type=\"{value}\" />")),
        });
    }
    cases.extend([
        WriteCase {
            what: "(h) control: another preserved value binds nothing (LIB-COMPAT-013)".to_owned(),
            input: format!(
                "<sacm:ArgumentPackage {h} xmlns:p=\"{P}\" xmlns:q=\"urn:q\" p:note=\"q:x\" \
                 xmi:id=\"a\"/>"
            ),
            found: vec![kept(169, "p:note", "ArgumentPackage", "a")],
            golden: empty_root_golden(&format!(
                "{r2} xmi:id=\"a\" xmlns:p=\"{P}\" p:note=\"q:x\" />"
            )),
        },
        WriteCase {
            what: "(i) an xsi:type kept on xmi:XMI".to_owned(),
            input: format!(
                "<xmi:XMI {h} xmlns:xsi=\"{XSI_NS}\" xmlns:p=\"{P}\" xsi:type=\"p:Model\">\
                 <sacm:ArgumentPackage xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>\
                 </sacm:ArgumentPackage></xmi:XMI>"
            ),
            found: vec![expected(
                Severity::Warning,
                WRAPPER_ATTRIBUTE_KEPT,
                loc(1, 194),
                &[],
                &[("attribute", "xsi:type"), ("element", "xmi:XMI")],
            )],
            golden: lines(&[
                DECLARATION,
                &format!(
                    "<xmi:XMI {h} xmlns:p=\"{P}\" xmlns:xsi=\"{XSI_NS}\" xsi:type=\"p:Model\">"
                ),
                "  <sacm:ArgumentPackage xmi:id=\"a\">",
                "    <name lang=\"en\" content=\"a\" />",
                "  </sacm:ArgumentPackage>",
                "</xmi:XMI>",
            ]),
        },
        WriteCase {
            what: "(k) a value with an empty prefix, which is no QName, under a default namespace"
                .to_owned(),
            input: format!(
                "<sacm:ArgumentPackage {h} xmlns=\"urn:d\" xmi:type=\":Bundle\" xmi:id=\"a\"/>"
            ),
            found: vec![kept(137, "xmi:type", "ArgumentPackage", "a")],
            golden: empty_root_golden(&format!("{r2} xmi:id=\"a\" xmi:type=\":Bundle\" />")),
        },
    ]);
    let mut documents = Vec::new();
    for case in &cases {
        documents.push(assert_case(case));
    }

    // (a) read back: the Warning is at the type on line 2.
    let what = "(a) read back";
    let back = read_with(what, &a_golden, &tolerant());
    assert_found(
        what,
        &back,
        &[kept_with(
            Severity::Warning,
            loc(2, 164),
            "xmi:type",
            "ArgumentPackage",
            "a",
        )],
    );

    // (j) A strict read keeps it too, with one Error, as the same document.
    let what = "(j) (a) read strictly";
    let strict = read_with(
        what,
        &a_input,
        &ReadOptions::default().with_mode(Mode::Strict),
    );
    assert_found(
        what,
        &strict,
        &[kept_with(
            Severity::Error,
            loc(1, 153),
            "xmi:type",
            "ArgumentPackage",
            "a",
        )],
    );
    assert!(
        strict.document() == &documents[0],
        "{ROW}: {what}: the strict document is not the tolerant one"
    );
}

// --- T2. A type in both XMI namespaces --------------------------------------------

/// One child of [`e_input`] typed in both XMI namespaces (or with
/// `xsi:type`): c1's kind (`None`: kept whole), whether c1 keeps content,
/// the diagnostics, and the written child's start tag.
struct ChildCase {
    what: &'static str,
    attributes: &'static str,
    kind: Option<ElementKind>,
    keeps: bool,
    found: Vec<Seen>,
    golden: String,
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one table of the row's child cases a to e, then the root case f"
)]
fn lib_compat_019_a_type_in_both_xmi_namespaces_is_read_from_xmi_2_5_1_and_written_once() {
    let emf = format!("xmlns:emf=\"{EMF_NS}\"");
    let cases = [
        ChildCase {
            what: "(a) emf:type then xmi:type, both Claim",
            attributes: "emf:type=\"sacm:Claim\" xmi:type=\"sacm:Claim\"",
            kind: Some(ElementKind::Claim),
            keeps: true,
            found: vec![
                kept(299, "emf:type", "Claim", "c1"),
                emf_read(299, "emf:type"),
            ],
            golden: e_golden(&format!(
                "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {emf} \
                 emf:type=\"sacm:Claim\">"
            )),
        },
        ChildCase {
            what: "(b) xmi:type then emf:type, both Claim",
            attributes: "xmi:type=\"sacm:Claim\" emf:type=\"sacm:Claim\"",
            kind: Some(ElementKind::Claim),
            keeps: true,
            found: vec![
                kept(321, "emf:type", "Claim", "c1"),
                emf_read(321, "emf:type"),
            ],
            golden: e_golden(&format!(
                "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {emf} \
                 emf:type=\"sacm:Claim\">"
            )),
        },
        ChildCase {
            what: "(c) emf:type Claim, xmi:type ArgumentReasoning",
            attributes: "emf:type=\"sacm:Claim\" xmi:type=\"sacm:ArgumentReasoning\"",
            kind: Some(ElementKind::ArgumentReasoning),
            keeps: true,
            found: vec![
                kept(299, "emf:type", "ArgumentReasoning", "c1"),
                emf_read(299, "emf:type"),
            ],
            golden: e_golden(&format!(
                "  <argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"c1\" {emf} \
                 emf:type=\"sacm:Claim\">"
            )),
        },
        ChildCase {
            what: "(d) emf:type and xsi:type, as today",
            attributes: "emf:type=\"sacm:Claim\" xsi:type=\"sacm:Claim\"",
            kind: Some(ElementKind::Claim),
            keeps: false,
            found: vec![
                emf_read(299, "emf:type"),
                expected(
                    Severity::Warning,
                    XSI_TYPE,
                    loc(1, 321),
                    &["c1"],
                    &[("class", "Claim"), ("id", "c1")],
                ),
            ],
            golden: e_golden("  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">"),
        },
        ChildCase {
            what: "(e) emf:type Claim, xmi:type a profile class: kept whole",
            attributes: "emf:type=\"sacm:Claim\" xmi:type=\"p:Proposition\"",
            kind: None,
            keeps: false,
            found: vec![element_kept(282), emf_read(299, "emf:type")],
            golden: lines(&[
                DECLARATION,
                &format!("{} xmi:id=\"a\">", r2()),
                "  <name lang=\"en\" content=\"a\" />",
                &format!(
                    "  <argumentElement {emf} xmlns:p=\"{P}\" xmlns:xsi=\"{XSI_NS}\" \
                     emf:type=\"sacm:Claim\" xmi:type=\"p:Proposition\" xmi:id=\"c1\">\
                     <name lang=\"en\" content=\"c\" /></argumentElement>"
                ),
                "</sacm:ArgumentPackage>",
            ]),
        },
    ];
    let mut documents = Vec::new();
    for case in &cases {
        let input = e_input(&c1(case.attributes));
        let output = read_with(case.what, &input, &tolerant());
        assert_found(case.what, &output, &case.found);
        let document = output.into_document();
        assert_eq!(
            (
                kind_of(&document, "c1"),
                document
                    .get("c1")
                    .is_some_and(SacmElement::has_preserved_content)
            ),
            (case.kind, case.keeps),
            "{ROW}: {}: c1's kind, and whether it keeps content",
            case.what
        );
        assert_writes_golden(case.what, &document, &tolerant(), &case.golden);
        documents.push(document);
    }
    assert!(
        documents[1] == documents[0],
        "{ROW}: (b) is not the document of (a)"
    );

    // (f) On a root, both are kept, and B does not apply.
    let case = WriteCase {
        what: "(f) a root with emf:type ArgumentPackage and xmi:type a profile class".to_owned(),
        input: r_input("emf:type=\"sacm:ArgumentPackage\" xmi:type=\"p:Bundle\""),
        found: vec![
            kept(188, "emf:type", "ArgumentPackage", "a"),
            emf_read(188, "emf:type"),
            kept(220, "xmi:type", "ArgumentPackage", "a"),
        ],
        golden: root_golden(&format!(
            "{} xmi:id=\"a\" {emf} xmlns:p=\"{P}\" emf:type=\"sacm:ArgumentPackage\" \
             xmi:type=\"p:Bundle\">",
            r2()
        )),
    };
    let document = assert_case(&case);
    assert_eq!(
        kind_of(&document, "a"),
        Some(ElementKind::ArgumentPackage),
        "{ROW}: {}: the root's kind",
        case.what
    );
}

// --- T3. An id, uuid or label in both XMI namespaces ------------------------------

#[test]
fn lib_compat_019_an_id_uuid_or_label_in_both_xmi_namespaces_is_read_from_xmi_2_5_1_and_written_once()
 {
    let emf = format!("xmlns:emf=\"{EMF_NS}\"");
    let child = |attributes: &str, id: &str| {
        format!(
            "<argumentElement xmi:type=\"sacm:Claim\" {attributes} xmi:id=\"{id}\">\
             <name lang=\"en\" content=\"c\"/></argumentElement>"
        )
    };
    // (a) to (c): (what, child, its ids, the EMF name, the written child).
    let cases = [
        (
            "(a) emf:id then xmi:id on a child",
            "<argumentElement xmi:type=\"sacm:Claim\" emf:id=\"c1\" xmi:id=\"c2\">\
             <name lang=\"en\" content=\"c\"/></argumentElement>"
                .to_owned(),
            "c2",
            "emf:id",
            format!(
                "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\" {emf} emf:id=\"c1\">"
            ),
        ),
        (
            "(b) emf:uuid then xmi:uuid on a child",
            child("emf:uuid=\"u1\" xmi:uuid=\"u2\"", "c1"),
            "c1",
            "emf:uuid",
            format!(
                "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {emf} xmi:uuid=\"u2\" \
                 emf:uuid=\"u1\">"
            ),
        ),
        (
            "(c) emf:label then xmi:label on a child",
            child("emf:label=\"l1\" xmi:label=\"l2\"", "c1"),
            "c1",
            "emf:label",
            format!(
                "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {emf} \
                 xmi:label=\"l2\" emf:label=\"l1\">"
            ),
        ),
    ];
    for (what, child, id, name, golden) in &cases {
        let case = WriteCase {
            what: (*what).to_owned(),
            input: e_input(child),
            found: vec![kept(321, name, "Claim", id), emf_read(321, name)],
            golden: e_golden(golden),
        };
        let document = assert_case(&case);
        assert_eq!(
            ids(&document),
            ["a", *id],
            "{ROW}: {what}: the ids of the elements"
        );
    }
    let what = "(a) emf:id then xmi:id on a child";
    let document = read_with(what, &e_input(&cases[0].1), &tolerant()).into_document();
    assert!(
        document.get("c1").is_none(),
        "{ROW}: {what}: EMF's id names an element"
    );

    // (d) On a root.
    let case = WriteCase {
        what: "(d) emf:id then xmi:id on a root".to_owned(),
        input: format!(
            "<sacm:ArgumentPackage {} {emf} emf:id=\"r1\" xmi:id=\"r2\">\
             <name lang=\"en\" content=\"a\"/></sacm:ArgumentPackage>",
            h()
        ),
        found: vec![
            kept(158, "emf:id", "ArgumentPackage", "r2"),
            emf_read(158, "emf:id"),
        ],
        golden: root_golden(&format!("{} xmi:id=\"r2\" {emf} emf:id=\"r1\">", r2())),
    };
    let document = assert_case(&case);
    let roots: Vec<&str> = document.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(roots, ["r2"], "{ROW}: {}: the roots", case.what);

    // (e) EMF's namespace bound to `xmi`, XMI 2.5.1's to `x`.
    let what = "(e) xmi:id in EMF's namespace, then x:id in XMI 2.5.1's";
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" \
         xmlns:x=\"{XMI_NS}\" xmi:id=\"p\"><name content=\"p\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2b\" x:id=\"c2\"/>\
         </sacm:ArgumentPackage>"
    );
    let document = read_with(what, &input, &tolerant()).into_document();
    assert_eq!(ids(&document), ["p", "c2"], "{ROW}: {what}: the ids");
    assert_writes_containing(
        what,
        &document,
        &tolerant(),
        &format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\" xmlns:ns1=\"{EMF_NS}\" \
             ns1:id=\"c2b\" />"
        ),
    );
}

// --- T4. An element keeping an EMF type -------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the row's cases a to g, children, roots and two moves, in order"
)]
fn lib_compat_019_an_element_keeping_an_emf_type_is_written_with_its_own_xmi_type() {
    let emf = format!("xmlns:emf=\"{EMF_NS}\"");
    // (a) to (c): children under the role argumentPackage, which declares
    // the kind ArgumentPackage.
    let cases = [
        (
            "(a) emf:type and xmi:type ArgumentPackage",
            "emf:type=\"sacm:ArgumentPackage\" xmi:type=\"sacm:ArgumentPackage\"",
            ElementKind::ArgumentPackage,
            format!(
                "  <argumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"p1\" {emf} \
                 emf:type=\"sacm:ArgumentPackage\">"
            ),
        ),
        (
            "(b) emf:type ArgumentPackageInterface, xmi:type ArgumentPackage",
            "emf:type=\"sacm:ArgumentPackageInterface\" xmi:type=\"sacm:ArgumentPackage\"",
            ElementKind::ArgumentPackage,
            format!(
                "  <argumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"p1\" {emf} \
                 emf:type=\"sacm:ArgumentPackageInterface\">"
            ),
        ),
        (
            "(c) emf:type ArgumentPackage, xmi:type ArgumentPackageInterface",
            "emf:type=\"sacm:ArgumentPackage\" xmi:type=\"sacm:ArgumentPackageInterface\"",
            ElementKind::ArgumentPackageInterface,
            format!(
                "  <argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"p1\" \
                 {emf} emf:type=\"sacm:ArgumentPackage\">"
            ),
        ),
    ];
    let mut first = None;
    for (what, attributes, kind, golden) in &cases {
        let case = WriteCase {
            what: (*what).to_owned(),
            input: ac_input(&p1(attributes)),
            found: vec![
                kept(253, "emf:type", kind.name(), "p1"),
                emf_read(253, "emf:type"),
            ],
            golden: ac_golden(golden),
        };
        let document = assert_case(&case);
        let back = read_with(what, &tolerant_write(what, &document), &tolerant()).into_document();
        assert_eq!(
            (kind_of(&document, "p1"), kind_of(&back, "p1")),
            (Some(*kind), Some(*kind)),
            "{ROW}: {what}: p1's kind, read and read back"
        );
        first.get_or_insert(document);
    }
    let Some(a_document) = first else {
        panic!("{ROW}: setup: no case (a)");
    };

    // (d) A root keeping emf:type beside its own xmi:type.
    let case = WriteCase {
        what: "(d) a root with xmi:type ArgumentPackage and emf:type a profile class".to_owned(),
        input: r_input("xmi:type=\"sacm:ArgumentPackage\" emf:type=\"p:Bundle\""),
        found: vec![
            kept(220, "emf:type", "ArgumentPackage", "a"),
            emf_read(220, "emf:type"),
        ],
        golden: root_golden(&format!(
            "{} xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"a\" {emf} xmlns:p=\"{P}\" \
             emf:type=\"p:Bundle\">",
            r2()
        )),
    };
    assert_case(&case);

    // (e) A root in the EMF dialect only, whose `xmi` is EMF's.
    let case = WriteCase {
        what: "(e) an EMF root whose xmi:type names a profile class".to_owned(),
        input: format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" \
             xmlns:p=\"{P}\" xmi:type=\"p:Bundle\" xmi:id=\"a\"/>"
        ),
        found: vec![
            expected(
                Severity::Warning,
                PREFIX_RENAMED,
                loc(1, 1),
                &["a"],
                &[
                    ("attribute", "xmi:type"),
                    ("element", "sacm:ArgumentPackage"),
                    ("prefix", "xmi"),
                    ("namespace", EMF_NS),
                    ("pinned_namespace", XMI_NS),
                    ("renamed_prefix", "ns1"),
                    ("holder_class", "ArgumentPackage"),
                    ("holder_id", "a"),
                ],
            ),
            emf_read(139, "xmi:type"),
        ],
        golden: empty_root_golden(&format!(
            "{} xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"a\" xmlns:ns1=\"{EMF_NS}\" \
             xmlns:p=\"{P}\" ns1:type=\"p:Bundle\" />",
            r2()
        )),
    };
    assert_case(&case);

    // (f) A child moved to the roots.
    let what = "(f) (a)'s p1 moved to the roots";
    let mut document = a_document;
    edit::apply(&mut document, MoveElement::root("p1"))
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the move is refused: {error}"));
    let golden = lines(&[
        DECLARATION,
        &wrapper_start(),
        "  <sacm:AssuranceCasePackage xmi:id=\"case\">",
        "    <name lang=\"en\" content=\"c\" />",
        "  </sacm:AssuranceCasePackage>",
        &format!(
            "  <sacm:ArgumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"p1\" {emf} \
             emf:type=\"sacm:ArgumentPackage\">"
        ),
        "    <name lang=\"en\" content=\"p\" />",
        "  </sacm:ArgumentPackage>",
        "</xmi:XMI>",
    ]);
    assert_writes_golden(what, &document, &tolerant(), &golden);

    // (g) A root keeping emf:type moved into a role.
    let what = "(g) M1's e moved under the case";
    let mut document = read_with(what, &m1(), &tolerant()).into_document();
    edit::apply(&mut document, MoveElement::new("e", "case"))
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the move is refused: {error}"));
    assert_writes_containing(
        what,
        &document,
        &tolerant(),
        &format!(
            "<argumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"e\" {emf} \
             xmlns:p=\"{P}\" emf:type=\"p:Bundle\">"
        ),
    );
}

// --- T5. The edits refuse to make a child of a root keeping a type ----------------

/// Applies `operation` to a copy of `document`, which must refuse it as
/// `KeptRootType` with `wanted` as `(element, attribute, value)`, leave it
/// `==` with the same revision; gives the refusal.
fn assert_kept_root_type(
    what: &str,
    document: &Document,
    operation: impl Into<Operation>,
    wanted: (&str, &str, &str),
) -> EditError {
    let mut edited = document.clone();
    let revision = document.revision();
    let error = match edit::apply(&mut edited, operation) {
        Ok(mutation) => panic!("{ROW}: {what}: the edit is accepted: {mutation:?}"),
        Err(error) => error,
    };
    let EditError::KeptRootType {
        element,
        attribute,
        value,
        ..
    } = &error
    else {
        panic!("{ROW}: {what}: not KeptRootType but {error:?} ({error})");
    };
    assert_eq!(
        (element.as_str(), attribute.as_str(), value.as_str()),
        wanted,
        "{ROW}: {what}: the element, attribute and value of the refusal"
    );
    assert_eq!(
        (error.code(), error.code().as_str()),
        (Code::CMD_INVALID_PARENT, "SACM-CMD-005"),
        "{ROW}: {what}: the code of the refusal"
    );
    assert!(
        edited == *document,
        "{ROW}: {what}: the refused edit changes the document"
    );
    assert_eq!(
        edited.revision(),
        revision,
        "{ROW}: {what}: the refused edit changes the revision"
    );
    error
}

/// Applies `operation` to a copy of `document`, which must refuse it with
/// a refusal `is` accepts.
fn assert_refused(
    what: &str,
    document: &Document,
    operation: impl Into<Operation>,
    is: impl Fn(&EditError) -> bool,
) {
    match edit::apply(&mut document.clone(), operation) {
        Ok(mutation) => panic!("{ROW}: {what}: the edit is accepted: {mutation:?}"),
        Err(error) => assert!(is(&error), "{ROW}: {what}: the refusal is {error:?}"),
    }
}

/// `document` with `operation` applied, which must be accepted.
fn applied(what: &str, document: &Document, operation: impl Into<Operation>) -> Document {
    let mut edited = document.clone();
    edit::apply(&mut edited, operation)
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the edit is refused: {error}"));
    edited
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "M1's read, its seven refusals, check and preview, the order controls and the \
              three allowed edits, in the order of the acceptance"
)]
fn lib_compat_019_a_root_keeping_an_xmi_or_xsi_type_is_not_moved_or_copied_into_a_role() {
    let input = m1();
    let output = read_with("M1", &input, &tolerant());
    assert_found(
        "M1",
        &output,
        &[
            kept(349, "xmi:type", "ArgumentPackage", "a"),
            kept(454, "xsi:type", "ArgumentPackage", "s"),
            kept(559, "emf:type", "ArgumentPackage", "e"),
            emf_read(559, "emf:type"),
            kept(664, "xmi:type", "ArgumentPackage", "i"),
            kept(967, "xmi:type", "ArgumentPackageBinding", "b"),
        ],
    );
    let document = output.into_document();

    // The refusals.
    let bundle = ("a", "xmi:type", "p:Bundle");
    let refused: [(&str, Operation, (&str, &str, &str)); 6] = [
        (
            "a under the case",
            MoveElement::new("a", "case").into(),
            bundle,
        ),
        (
            "a under the package x",
            MoveElement::new("a", "x").into(),
            bundle,
        ),
        (
            "s, keeping xsi:type, under the case",
            MoveElement::new("s", "case").into(),
            ("s", "xsi:type", "p:Bundle"),
        ),
        (
            "i, keeping a SACM class, under the case",
            MoveElement::new("i", "case").into(),
            ("i", "xmi:type", "sacm:ArgumentPackageInterface"),
        ),
        (
            "the binding b under the case",
            MoveElement::new("b", "case").into(),
            ("b", "xmi:type", "p:X"),
        ),
        (
            "a under the case at a position past the end (step 5 before 6)",
            MoveElement::new("a", "case").with_position(9).into(),
            bundle,
        ),
    ];
    for (what, operation, wanted) in refused {
        assert_kept_root_type(what, &document, operation, wanted);
    }
    let what = "a copy of x and a under the case";
    let copy = CopyElements::new(&document, ["x", "a"], "case")
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the copy is not made: {error}"));
    assert_kept_root_type(what, &document, copy, bundle);

    // check and preview agree with apply.
    let what = "a under the case, checked and previewed";
    let operation = Operation::from(MoveElement::new("a", "case"));
    let error = assert_kept_root_type(what, &document, operation.clone(), bundle);
    assert_eq!(
        edit::check(&document, &operation),
        Err(error.clone()),
        "{ROW}: {what}: check answers otherwise than apply"
    );
    assert_eq!(
        edit::preview(&document, &operation).err(),
        Some(error),
        "{ROW}: {what}: preview answers otherwise than apply"
    );

    // The order: an unknown parent and a parent in the subtree come first.
    assert_refused(
        "a under an unknown parent",
        &document,
        MoveElement::new("a", "nope"),
        |error| matches!(error, EditError::ParentNotFound { .. }),
    );
    assert_refused(
        "a under itself",
        &document,
        MoveElement::new("a", "a"),
        |error| matches!(error, EditError::ParentInSubtree { .. }),
    );

    // Allowed: a move among the roots, a copy to the roots, a declared
    // subclass.
    let what = "a moved to the first root";
    let moved = applied(what, &document, MoveElement::root("a").with_position(0));
    assert_writes_back(what, &moved, &tolerant());
    let what = "a copied to the roots";
    let copy = CopyElements::root(&document, ["a"])
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the copy is not made: {error}"));
    let copied = applied(what, &document, copy);
    assert_writes_containing(
        what,
        &copied,
        &tolerant(),
        &format!(
            "  <sacm:ArgumentPackage xmi:id=\"argument_package_1\" xmlns:p=\"{P}\" \
             xmi:type=\"p:Bundle\">"
        ),
    );
    let what = "a, a declared subclass, under the case";
    let declared = tolerant().with_subclass(P, "Bundle", ElementKind::ArgumentPackage);
    let subclassed = read_with(what, &input, &declared).into_document();
    let moved = applied(what, &subclassed, MoveElement::new("a", "case"));
    assert_writes_containing(
        what,
        &moved,
        &declared,
        &format!("<argumentPackage xmi:type=\"p:Bundle\" xmi:id=\"a\" xmlns:p=\"{P}\">"),
    );
}

// --- T6. The refusal's code, message and JSON -------------------------------------

#[test]
fn lib_compat_019_the_kept_root_type_refusal_has_its_code_message_and_json() {
    let document = read_with("M1", &m1(), &tolerant()).into_document();
    let mut errors = Vec::new();
    for (id, attribute, value) in [
        ("a", "xmi:type", "p:Bundle"),
        ("s", "xsi:type", "p:Bundle"),
        ("i", "xmi:type", "sacm:ArgumentPackageInterface"),
    ] {
        let what = format!("{id} under the case");
        let error = assert_kept_root_type(
            &what,
            &document,
            MoveElement::new(id, "case"),
            (id, attribute, value),
        );
        assert_eq!(
            error.to_string(),
            format!(
                "element \"{id}\" keeps the type attribute {attribute}=\"{value}\", which only \
                 a root can carry, so it cannot be a child"
            ),
            "{ROW}: {what}: the Display of the refusal"
        );
        errors.push(error);
    }
    assert_json(&errors[0]);
}

/// With `serde`, `error` (a's refusal) is written as the `KeptRootType`
/// sample of `edit-errors.json`, its last, and reads back equal.
#[cfg(feature = "serde")]
fn assert_json(error: &EditError) {
    let written = serde_json::to_string(error)
        .unwrap_or_else(|failure| panic!("{ROW}: {error:?} does not serialize: {failure}"));
    assert_eq!(
        written, KEPT_ROOT_TYPE_JSON,
        "{ROW}: the refusal is not written as its JSON sample"
    );
    let read: EditError = serde_json::from_str(KEPT_ROOT_TYPE_JSON)
        .unwrap_or_else(|failure| panic!("{ROW}: the JSON sample does not read: {failure}"));
    assert_eq!(&read, error, "{ROW}: the JSON sample does not read back");
    let golden = std::fs::read_to_string(EDIT_ERRORS_JSON)
        .unwrap_or_else(|failure| panic!("{ROW}: setup: cannot read edit-errors.json: {failure}"));
    let samples: Vec<EditError> = serde_json::from_str(&golden)
        .unwrap_or_else(|failure| panic!("{ROW}: edit-errors.json does not read: {failure}"));
    assert_eq!(
        samples.last(),
        Some(error),
        "{ROW}: the last sample of edit-errors.json is not the refusal"
    );
}

/// Without `serde`, there is no JSON to check.
#[cfg(not(feature = "serde"))]
fn assert_json(_error: &EditError) {}

// --- Edges of rule V and of an id in both XMI namespaces --------------------------

/// The column of the first `needle` in the one-line `input`.
fn column_of(input: &str, needle: &str) -> u32 {
    let Some(offset) = input.find(needle) else {
        panic!("{ROW}: setup: {needle:?} is not in {input:?}");
    };
    u32::try_from(offset + 1).unwrap_or_else(|_| panic!("{ROW}: setup: {input:?} is too long"))
}

/// A kept type attribute's value binds only a prefix that it names as a
/// `QName` and that is bound to a namespace: not the empty default
/// namespace, not a value with an empty local part, and no other attribute
/// in the XMI namespace; a renamed value keeps the white space around it.
#[test]
fn lib_compat_019_a_kept_type_value_binds_only_a_bound_prefix_it_names_as_a_qname() {
    let h = h();
    let r2 = r2();
    let cases = [
        (
            "(a) an unprefixed value under the empty default namespace",
            format!("<sacm:ArgumentPackage {h} xmlns=\"\" xmi:type=\"Bundle\" xmi:id=\"a\"/>"),
            "xmi:type",
            format!("{r2} xmi:id=\"a\" xmi:type=\"Bundle\" />"),
        ),
        (
            "(b) a value with an empty local part, which is no QName",
            format!("<sacm:ArgumentPackage {h} xmlns:p=\"{P}\" xmi:type=\"p:\" xmi:id=\"a\"/>"),
            "xmi:type",
            format!("{r2} xmi:id=\"a\" xmi:type=\"p:\" />"),
        ),
        (
            "(c) control: an XMI attribute other than type binds nothing",
            format!("<sacm:ArgumentPackage {h} xmlns:q=\"urn:q\" xmi:foo=\"q:x\" xmi:id=\"a\"/>"),
            "xmi:foo",
            format!("{r2} xmi:id=\"a\" xmi:foo=\"q:x\" />"),
        ),
        (
            "(d) a renamed value keeps the white space around it",
            format!(
                "<sacm:ArgumentPackage {h} xmlns=\"urn:d\" xmi:type=\" Bundle \" xmi:id=\"a\"/>"
            ),
            "xmi:type",
            format!("{r2} xmi:id=\"a\" xmlns:ns1=\"urn:d\" xmi:type=\" ns1:Bundle \" />"),
        ),
    ];
    for (what, input, attribute, start) in cases {
        let column = column_of(&input, &format!("{attribute}="));
        assert_case(&WriteCase {
            what: what.to_owned(),
            found: vec![kept(column, attribute, "ArgumentPackage", "a")],
            input,
            golden: empty_root_golden(&start),
        });
    }
}

/// An EMF fragment path that names a preserved element carrying `id` in
/// both XMI namespaces is read as its XMI 2.5.1 `xmi:id`, the one in the
/// document's id space, not as EMF's (R4 (c), ADR 0044 amendment 1).
#[test]
fn lib_compat_019_a_path_names_a_preserved_element_by_its_xmi_2_5_1_id() {
    let what = "a path to an Artifact kept under argumentElement, with emf:id and xmi:id";
    let input = format!(
        "<sacm:ArgumentPackage {} xmlns:emf=\"{EMF_NS}\" xmi:id=\"a\">\
         <name lang=\"en\" content=\"a\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\
         <argumentElement xmi:type=\"sacm:Artifact\" emf:id=\"k0\" xmi:id=\"k1\"/>\
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" \
         source=\"//@argumentElement.1\" target=\"c0\"/></sacm:ArgumentPackage>",
        h()
    );
    let document = read_with(what, &input, &tolerant()).into_document();
    let Some(inference) = document.get("i1") else {
        panic!("{ROW}: {what}: no i1");
    };
    let sources: Vec<&str> = inference.sources().iter().map(ElementId::as_str).collect();
    assert_eq!(sources, ["k1"], "{ROW}: {what}: the sources of i1");
    assert_writes_back(what, &document, &tolerant());
}

/// The kept-type refusal comes after `InvalidParent`, for a move and for a
/// copy (check 3 before 5; copy check 2, `InvalidParent` first), and only a
/// type attribute refuses: a root keeping another XMI 2.5.1 attribute moves
/// and copies into a role.
#[test]
fn lib_compat_019_a_kept_type_is_refused_after_invalid_parent_and_no_other_kept_attribute_is() {
    let input = format!(
        "<xmi:XMI {} xmlns:p=\"{P}\"><sacm:AssuranceCasePackage xmi:id=\"case\">\
         <name lang=\"en\" content=\"c\"/></sacm:AssuranceCasePackage>\
         <sacm:TerminologyPackage xmi:id=\"terms\"><name lang=\"en\" content=\"t\"/>\
         </sacm:TerminologyPackage>\
         <sacm:ArgumentPackage xmi:type=\"p:Bundle\" xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>\
         </sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:foo=\"p:x\" xmi:id=\"f\"><name lang=\"en\" content=\"f\"/>\
         </sacm:ArgumentPackage></xmi:XMI>",
        h()
    );
    let document = read_with("the case, terms, a and f", &input, &tolerant()).into_document();
    let invalid_parent = |error: &EditError| matches!(error, EditError::InvalidParent { .. });
    assert_refused(
        "a under the TerminologyPackage terms",
        &document,
        MoveElement::new("a", "terms"),
        invalid_parent,
    );
    let what = "a copy of a under the TerminologyPackage terms";
    let copy = CopyElements::new(&document, ["a"], "terms")
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the copy is not made: {error}"));
    assert_refused(what, &document, copy, invalid_parent);

    let what = "f, keeping xmi:foo, moved under the case";
    let moved = applied(what, &document, MoveElement::new("f", "case"));
    assert_writes_back(what, &moved, &tolerant());
    let what = "f, keeping xmi:foo, copied under the case";
    let copy = CopyElements::new(&document, ["f"], "case")
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the copy is not made: {error}"));
    let copied = applied(what, &document, copy);
    assert_writes_back(what, &copied, &tolerant());
}

/// The `xmi:type` of `xmi:XMI`, XMI metadata that both saves write back,
/// keeps the binding of the prefix its value names, as a preserved type
/// does; one in EMF's namespace, held as XMI 2.5.1's, too (ADR 0042, ADR
/// 0044 D1).
#[test]
fn lib_compat_019_a_metadata_type_value_keeps_the_binding_of_its_prefix() {
    let root = "<sacm:ArgumentPackage xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>\
                </sacm:ArgumentPackage>";
    let golden = lines(&[
        DECLARATION,
        &format!("<xmi:XMI {} xmlns:p=\"{P}\" xmi:type=\"p:Model\">", h()),
        "  <sacm:ArgumentPackage xmi:id=\"a\">",
        "    <name lang=\"en\" content=\"a\" />",
        "  </sacm:ArgumentPackage>",
        "</xmi:XMI>",
    ]);

    // (a) In XMI 2.5.1's namespace: no diagnostic, in either mode, and a
    // strict write writes it as a tolerant one does.
    let what = "(a) xmi:type on xmi:XMI";
    let input = format!(
        "<xmi:XMI {} xmlns:p=\"{P}\" xmi:type=\"p:Model\">{root}</xmi:XMI>",
        h()
    );
    let document = assert_case(&WriteCase {
        what: what.to_owned(),
        input: input.clone(),
        found: Vec::new(),
        golden: golden.clone(),
    });
    let strict = ReadOptions::default().with_mode(Mode::Strict);
    let output = read_with(what, &input, &strict);
    assert_found(what, &output, &[]);
    assert!(
        output.document() == &document,
        "{ROW}: {what}: the strict document is not the tolerant one"
    );
    let written = xmi::write(&document, &WriteOptions::default().with_mode(Mode::Strict))
        .unwrap_or_else(|error| panic!("{ROW}: {what}: a strict write refuses it: {error}"));
    assert_eq!(written, golden, "{ROW}: {what}: the strict write");

    // (b) In EMF's namespace, read as XMI 2.5.1's in tolerant mode.
    let input = format!(
        "<xmi:XMI xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" xmlns:p=\"{P}\" \
         xmi:type=\"p:Model\">{root}</xmi:XMI>"
    );
    assert_case(&WriteCase {
        what: "(b) xmi:type on xmi:XMI, both in EMF's namespace".to_owned(),
        found: vec![emf_read(1, "xmi:XMI")],
        input,
        golden,
    });
}

/// The binding a type value keeps counts against the size limit, as every
/// kept binding does (ADR 0041 D2): on a root's kept `type` and on the
/// `xmi:type` of `xmi:XMI` in EMF's namespace, each holder counts it once.
#[test]
fn lib_compat_019_a_type_values_binding_counts_against_the_size_limit() {
    let what = "a long namespace named by two type values";
    let long = format!("urn:example:profile:{}", "x".repeat(20_000));
    let input = format!(
        "<xmi:XMI xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" xmlns:p=\"{long}\" \
         xmi:type=\"p:Model\"><sacm:ArgumentPackage xmi:type=\"p:Bundle\" xmi:id=\"a\">\
         <name lang=\"en\" content=\"{}\"/></sacm:ArgumentPackage></xmi:XMI>",
        "a".repeat(2_000)
    );
    let size = u64::try_from(input.len()).unwrap_or_else(|_| panic!("{ROW}: setup: the size"));
    // Within the input's size, the two bindings of `p` pass it at the
    // second type.
    let limited = tolerant().with_limits(Limits::default().with_max_input_bytes(size));
    let Err(error) = xmi::read(input.as_bytes(), &limited) else {
        panic!("{ROW}: {what}: the read is not refused");
    };
    assert_eq!(
        (
            matches!(error, ReadError::LimitExceeded { .. }),
            error.template(),
            error.location()
        ),
        (
            true,
            "SACM-SEC-003/kept-namespaces",
            Some(loc(1, column_of(&input, "xmi:type=\"p:Bundle\"")))
        ),
        "{ROW}: {what}: the refusal: {error:?}"
    );
    // With room for them, it reads and writes back.
    let roomy = tolerant().with_limits(Limits::default().with_max_input_bytes(size * 4));
    let document = read_with(what, &input, &roomy).into_document();
    assert_writes_back(what, &document, &roomy);
}

// --- Verifier round 1: a version in both XMI namespaces, an unprefixed subclass ---

/// An owned part carrying `id` in both XMI namespaces is checked by its XMI
/// 2.5.1 `xmi:id`, not by the first in document order: EMF's, a duplicate of
/// the root's id, refuses nothing, and the part, which keeps no attribute of
/// its own, is kept whole (ADR 0044 amendment 1; verifier round 1, a
/// surviving mutation of `read_owned_id`).
#[test]
fn lib_compat_019_an_owned_part_with_an_id_in_both_xmi_namespaces_is_checked_by_its_xmi_2_5_1_id() {
    let what = "a root's name with emf:id=\"a\" before xmi:id=\"n2\"";
    let input = format!(
        "<sacm:ArgumentPackage {} xmlns:emf=\"{EMF_NS}\" xmi:id=\"a\">\
         <name emf:id=\"a\" xmi:id=\"n2\" lang=\"en\" content=\"a\"/></sacm:ArgumentPackage>",
        h()
    );
    let output = read_with(what, &input, &tolerant());
    assert_found(
        what,
        &output,
        &[
            expected(
                Severity::Warning,
                ELEMENT_KEPT,
                loc(1, column_of(&input, "<name")),
                &["a"],
                &[
                    ("element", "name"),
                    ("holder_class", "ArgumentPackage"),
                    ("holder_id", "a"),
                ],
            ),
            emf_read(column_of(&input, "emf:id="), "emf:id"),
        ],
    );
    let document = output.into_document();
    assert_eq!(ids(&document), ["a"], "{ROW}: {what}: the ids");
    assert_writes_back(what, &document, &tolerant());
}

/// A `version` in both XMI namespaces, on a root and on `xmi:XMI`, in either
/// order, is read as the version and ignored, both of them, with one
/// SACM-XMI-011 Info each, and nothing is kept: the write carries neither,
/// reads back `==` and writes again byte-identical (D-XMI-VERSION, ADR 0044
/// amendment 1; verifier round 1, B1).
#[test]
fn lib_compat_019_a_version_in_both_xmi_namespaces_is_read_and_ignored_twice() {
    const ON_ROOT: &str = "SACM-XMI-011/version-on-root";
    const ON_XMI: &str = "SACM-XMI-011/version-on-xmi-element";
    let h = h();
    let emf = format!("xmlns:emf=\"{EMF_NS}\"");
    let on_root = |input: &str, attribute: &str, value: &str| {
        expected(
            Severity::Info,
            ON_ROOT,
            loc(1, column_of(input, &format!("{attribute}="))),
            &["a"],
            &[("value", value), ("class", "ArgumentPackage"), ("id", "a")],
        )
    };
    let on_xmi = |input: &str, attribute: &str, value: &str| {
        expected(
            Severity::Info,
            ON_XMI,
            loc(1, column_of(input, &format!("{attribute}="))),
            &[],
            &[("value", value)],
        )
    };
    let wrapper_golden = lines(&[
        DECLARATION,
        &wrapper_start(),
        "  <sacm:ArgumentPackage xmi:id=\"a\" />",
        "  <sacm:ArgumentPackage xmi:id=\"b\" />",
        "</xmi:XMI>",
    ]);
    for (order, versions, emf_first) in [
        (
            "emf:version first",
            "emf:version=\"2.0\" xmi:version=\"2.5.1\"",
            true,
        ),
        (
            "xmi:version first",
            "xmi:version=\"2.5.1\" emf:version=\"2.0\"",
            false,
        ),
    ] {
        // On a root.
        let input = format!("<sacm:ArgumentPackage {h} {emf} {versions} xmi:id=\"a\"/>");
        let emf_version = on_root(&input, "emf:version", "2.0");
        let xmi_version = on_root(&input, "xmi:version", "2.5.1");
        let emf_name = emf_read(column_of(&input, "emf:version="), "emf:version");
        let found = if emf_first {
            vec![emf_version, emf_name, xmi_version]
        } else {
            vec![xmi_version, emf_version, emf_name]
        };
        assert_case(&WriteCase {
            what: format!("a root, {order}"),
            input,
            found,
            golden: empty_root_golden(&format!("{} xmi:id=\"a\" />", r2())),
        });

        // On xmi:XMI.
        let input = format!(
            "<xmi:XMI {h} {emf} {versions}><sacm:ArgumentPackage xmi:id=\"a\"/>\
             <sacm:ArgumentPackage xmi:id=\"b\"/></xmi:XMI>"
        );
        let emf_version = on_xmi(&input, "emf:version", "2.0");
        let xmi_version = on_xmi(&input, "xmi:version", "2.5.1");
        let emf_name = emf_read(column_of(&input, "emf:version="), "emf:version");
        let found = if emf_first {
            vec![emf_version, emf_name, xmi_version]
        } else {
            vec![xmi_version, emf_version, emf_name]
        };
        assert_case(&WriteCase {
            what: format!("xmi:XMI, {order}"),
            input,
            found,
            golden: wrapper_golden.clone(),
        });
    }
}

/// An unprefixed type value is resolved in the default namespace in scope,
/// as XML Schema resolves a `QName` and as a SACM class in a type value is
/// (SACM23-XMI-002), also when it names a subclass the application declared
/// with `ReadOptions::with_subclass` (LIB-COMPAT-017; ADR 0044 amendment 1;
/// verifier round 1, B2). Under a default namespace bound to the declared
/// namespace, a root reads as the subclass, written with the holder's `nsN`,
/// and reads back `==` with the same options. With no default namespace, or
/// `xmlns=""`, the value is in no namespace and names no subclass, even one
/// declared in the empty namespace. A role element under a default namespace
/// is itself in it, so it is no role element and is kept whole (ADR 0033):
/// on a child, a default namespace never names a subclass.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one table of the root cases, then the child cases, each with its diagnostics"
)]
fn lib_compat_019_an_unprefixed_type_value_names_a_declared_subclass_in_the_default_namespace() {
    const SUBCLASS_KEPT: &str = "SACM-XMI-003/subclass-type-kept";
    let h = h();
    let r2 = r2();
    let bundle =
        ReadOptions::default().with_subclass("urn:d", "Bundle", ElementKind::ArgumentPackage);
    let empty_bundle =
        ReadOptions::default().with_subclass("", "Bundle", ElementKind::ArgumentPackage);
    let subclass_of = |document: &Document, id: &str| {
        document
            .get(id)
            .and_then(SacmElement::subclass)
            .map(|class| (class.namespace().to_owned(), class.name().to_owned()))
    };

    // (a) A root under a default namespace bound to the declared one.
    let what = "(a) a root's unprefixed type under the declared default namespace";
    let input =
        format!("<sacm:ArgumentPackage {h} xmlns=\"urn:d\" xmi:type=\"Bundle\" xmi:id=\"a\"/>");
    let output = read_with(what, &input, &bundle);
    assert_found(
        what,
        &output,
        &[expected(
            Severity::Warning,
            SUBCLASS_KEPT,
            loc(1, column_of(&input, "xmi:type=")),
            &["a"],
            &[
                ("type", "Bundle"),
                ("class", "ArgumentPackage"),
                ("id", "a"),
            ],
        )],
    );
    let document = output.into_document();
    assert_eq!(
        (kind_of(&document, "a"), subclass_of(&document, "a")),
        (
            Some(ElementKind::ArgumentPackage),
            Some(("urn:d".to_owned(), "Bundle".to_owned()))
        ),
        "{ROW}: {what}: a is not an ArgumentPackage with the subclass Bundle of urn:d"
    );
    assert_writes_golden(
        what,
        &document,
        &bundle,
        &empty_root_golden(&format!(
            "{r2} xmi:type=\"ns1:Bundle\" xmi:id=\"a\" xmlns:ns1=\"urn:d\" />"
        )),
    );

    // (b) to (d) A root whose unprefixed type is in no namespace: kept as an
    // attribute, as without the declarations.
    let no_default = format!("<sacm:ArgumentPackage {h} xmi:type=\"Bundle\" xmi:id=\"a\"/>");
    let undeclared =
        format!("<sacm:ArgumentPackage {h} xmlns=\"\" xmi:type=\"Bundle\" xmi:id=\"a\"/>");
    for (what, input, declared) in [
        ("(b) no default namespace", &no_default, &bundle),
        (
            "(c) no default namespace, a subclass declared in the empty namespace",
            &no_default,
            &empty_bundle,
        ),
        (
            "(d) xmlns=\"\", a subclass declared in the empty namespace",
            &undeclared,
            &empty_bundle,
        ),
    ] {
        let output = read_with(what, input, declared);
        assert_found(
            what,
            &output,
            &[kept(
                column_of(input, "xmi:type="),
                "xmi:type",
                "ArgumentPackage",
                "a",
            )],
        );
        let document = output.into_document();
        assert_eq!(
            subclass_of(&document, "a"),
            None,
            "{ROW}: {what}: a has a subclass"
        );
        assert_writes_golden(
            what,
            &document,
            declared,
            &empty_root_golden(&format!("{r2} xmi:id=\"a\" xmi:type=\"Bundle\" />")),
        );
    }

    // (e) to (g) A child: in the default namespace it is no role element; in
    // no namespace its unprefixed type is in none and names no subclass. Each
    // is kept whole, as without the declarations.
    let proposition = ReadOptions::default().with_subclass("urn:d", "Prop", ElementKind::Claim);
    let empty_proposition = ReadOptions::default().with_subclass("", "Prop", ElementKind::Claim);
    for (what, input, declared) in [
        (
            "(e) a child under the declared default namespace",
            format!(
                "<sacm:ArgumentPackage {h} xmi:id=\"a\"><argumentElement xmlns=\"urn:d\" \
                 xmi:type=\"Prop\" xmi:id=\"c1\"/></sacm:ArgumentPackage>"
            ),
            &proposition,
        ),
        (
            "(f) a child with no default namespace, a subclass declared in the empty namespace",
            format!(
                "<sacm:ArgumentPackage {h} xmi:id=\"a\"><argumentElement xmi:type=\"Prop\" \
                 xmi:id=\"c1\"/></sacm:ArgumentPackage>"
            ),
            &empty_proposition,
        ),
        (
            "(g) a child with xmlns=\"\" under a default namespace, a subclass declared in the \
             empty namespace",
            format!(
                "<sacm:ArgumentPackage {h} xmlns=\"urn:d\" xmi:id=\"a\"><argumentElement \
                 xmlns=\"\" xmi:type=\"Prop\" xmi:id=\"c1\"/></sacm:ArgumentPackage>"
            ),
            &empty_proposition,
        ),
    ] {
        let output = read_with(what, &input, declared);
        assert_found(
            what,
            &output,
            &[element_kept(column_of(&input, "<argumentElement"))],
        );
        let document = output.into_document();
        assert_eq!(
            kind_of(&document, "c1"),
            None,
            "{ROW}: {what}: c1 is read, not kept whole"
        );
        assert_writes_back(what, &document, declared);
    }
}
