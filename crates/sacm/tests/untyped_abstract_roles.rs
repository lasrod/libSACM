//! Tests for an untyped child under a role whose declared type is abstract
//! (LIB-COMPAT-010; XMI 2.5.1 §7.6.3; ADR 0022 and its amendment 1).
//!
//! An element under such a role with no `xmi:type` or `xsi:type` is refused
//! in both modes with SACM-XMI-005 at its start tag: its local name is the
//! role, so no class can be inferred, and none is guessed. In this release
//! the one readable role with an abstract declared type is `argumentElement`
//! of an `ArgumentPackage` (§11.4, `ArgumentationElement`).
//!
//! A role element in reference form, carrying `xmi:idref` or an unqualified
//! `href`, is a proxy, not an untyped object: typed or untyped, under an
//! abstract or a concrete role. A proxy of an element of the same document
//! is SACM23-XMI-030's since slice 0073a: of no root, it is kept with
//! `SACM-XMI-003/proxy-of-no-root`. One of another document is refused as
//! not read yet, naming SACM23-XMI-029 (slice 0068 splits it from
//! SACM23-XMI-027, ADR 0040). Until slice 0073a an `xmi:idref` named
//! SACM23-XMI-019.
//!
//! Assumptions this file makes about the design, which the lead confirms:
//! - The code is `Code::XMI_MISSING_TYPE` (SACM-XMI-005), named like
//!   `XMI_MISSING_ID` (SACM-XMI-004).
//! - The refusal is a coded `ReadError` variant of its own (ADR 0015
//!   amendment 2). Its name is not pinned: the tests use `code()`,
//!   `location()` and `locations()`, and that it is not `Unsupported`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::model::{Document, ElementId, ElementKind, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "LIB-COMPAT-010";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where the refusal is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// An ArgumentPackage root `p` named "P", holding `body` on line 3.
fn ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// An AssuranceCasePackage root `case` named "Case", holding `body` on line 3.
fn acp(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  {body}\n\
         </sacm:AssuranceCasePackage>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
            continue;
        }
        text.push(c);
        match c {
            '\n' if after_cr => {}
            '\n' | '\r' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        after_cr = c == '\r';
    }
    (text, marks)
}

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read to `==`
/// documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => documents.push(output.into_document()),
            Err(error) => {
                panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `template` without its one mark in strict mode and asserts that it
/// is refused with SACM-XMI-005 at the mark. A tolerant read preserves the
/// child in its parent, with SACM-XMI-005 as a Warning (LIB-COMPAT-001,
/// ADR 0035).
fn assert_missing_type_at_mark(what: &str, template: &str) {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
    let error = &match read(Mode::Strict, &input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    };
    assert!(
        !matches!(error, ReadError::Unsupported { .. }),
        "{ROW}: {what}: still Unsupported: {error:?} ({error})"
    );
    assert_eq!(
        (error.code(), error.location(), error.locations()),
        (Code::XMI_MISSING_TYPE, Some(marks[0]), marks.as_slice()),
        "{ROW}: {what}: not SACM-XMI-005 at the start tag: {error:?} ({error})"
    );
    let shown = error.to_string();
    assert!(
        shown.contains(&marks[0].to_string()) && !shown.contains('\n'),
        "{ROW}: {what}: the refusal {shown:?} is not one line showing {}",
        marks[0]
    );
}

#[test]
fn lib_compat_010_an_untyped_child_under_an_abstract_role_is_sacm_xmi_005_at_its_start_tag() {
    let cases = [
        (
            "an untyped argumentElement",
            ap(&format!("{MARK}<argumentElement xmi:id=\"c1\"/>")),
        ),
        (
            "an untyped argumentElement with a name",
            ap(&format!(
                "{MARK}<argumentElement xmi:id=\"c1\"><name content=\"C\"/></argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement with children",
            ap(&format!(
                "{MARK}<argumentElement xmi:id=\"c1\"><name content=\"C\"/>\
                 <description xmi:id=\"d\"/><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"/>\
                 </argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement without an xmi:id",
            ap(&format!(
                "{MARK}<argumentElement><name content=\"C\"/></argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement with attributes over two lines",
            ap(&format!(
                "{MARK}<argumentElement xmi:id=\"c1\"\n      assertionDeclaration=\"assumed\"/>"
            )),
        ),
        (
            "an untyped argumentElement after a typed one",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n  \
                 {MARK}<argumentElement xmi:id=\"c2\"/>"
            )),
        ),
        (
            "an untyped argumentElement in an argumentPackage of an AssuranceCasePackage",
            acp(&format!(
                "<argumentPackage xmi:id=\"a\"><name content=\"A\"/>\n    \
                 {MARK}<argumentElement xmi:id=\"c1\"/></argumentPackage>"
            )),
        ),
        (
            "an untyped argumentElement in a nested ArgumentPackage",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner\">\
                 <name content=\"I\"/>\n    {MARK}<argumentElement xmi:id=\"c1\"/></argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement in an xmi:XMI wrapper",
            format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                 xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n\
                 <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>\n  \
                 {MARK}<argumentElement xmi:id=\"c1\"/></sacm:ArgumentPackage>\n</xmi:XMI>"
            ),
        ),
    ];
    for (what, template) in &cases {
        assert_missing_type_at_mark(what, template);
    }
}

#[test]
fn lib_compat_010_a_reference_form_element_under_an_abstract_role_is_not_missing_type() {
    // A role element with `xmi:idref` or `href` is the reference form of a
    // composite (XMI 2.5.1 §7.10.1, a proxy), not an untyped object: no
    // class is missing, so it is no SACM-XMI-005. Since slice 0073a a proxy
    // of the same document (`xmi:idref`, or `href="#c1"`) whose id names no
    // root is kept with `SACM-XMI-003/proxy-of-no-root` at its start tag, in
    // both modes (SACM23-XMI-030); one of another document is held as a
    // proxy of its container since slice 0073b, with no diagnostic
    // (SACM23-XMI-029; ADR 0040 amendment 1). Until slice 0073a an
    // `xmi:idref` named SACM23-XMI-019, and until slice 0073b an `href` to
    // another document was `Unsupported` naming SACM23-XMI-029.
    let claim = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n  ";
    let kept = [
        (
            "an argumentElement xmi:idref in an ArgumentPackage",
            ap(&format!("{claim}{MARK}<argumentElement xmi:idref=\"c1\"/>")),
            "p",
        ),
        (
            "an argumentElement href=\"#c1\" in an ArgumentPackage",
            ap(&format!("{claim}{MARK}<argumentElement href=\"#c1\"/>")),
            "p",
        ),
        (
            "an argumentElement xmi:idref in an argumentPackage of an AssuranceCasePackage",
            acp(&format!(
                "<argumentPackage xmi:id=\"a\"><name content=\"A\"/>{claim}\
                 {MARK}<argumentElement xmi:idref=\"c1\"/></argumentPackage>"
            )),
            "a",
        ),
    ];
    for (what, template, holder) in &kept {
        assert_proxy_of_no_root_at_mark(ROW, what, template, holder);
    }
    assert_held_as_proxy(
        ROW,
        "an argumentElement href to another document in an ArgumentPackage",
        &ap(&format!("{claim}<argumentElement href=\"o.xmi#c1\"/>")),
        ("p", "argumentElement", "o.xmi#c1"),
    );
}

/// Reads `template` without its one mark in both modes and asserts that it
/// is read, keeping a same-file proxy whose id names no root: one
/// `SACM-XMI-003/proxy-of-no-root` at the mark, affecting `holder`, an Error
/// strict and a Warning tolerant, with no SACM-XMI-005 (SACM23-XMI-030; ADR
/// 0042 D8).
fn assert_proxy_of_no_root_at_mark(row: &str, what: &str, template: &str, holder: &str) {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{row}: setup: {what} has not one mark");
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!("{row}: {what}: a {mode:?} read refused it: {error:?} ({error})"),
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.template(),
                    found.location(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::XMI_UNKNOWN_CONTENT,
                severity,
                "SACM-XMI-003/proxy-of-no-root",
                Some(marks[0]),
                vec![ElementId::from(holder)]
            )],
            "{row}: {what}: not one proxy-of-no-root at the start tag affecting {holder:?} in \
             {mode:?} mode"
        );
    }
}

/// Reads `template` in both modes and asserts that it is read, holding the
/// role element in reference form as a cross-file proxy of the element
/// `holder`: `held` is the holder's id and the one proxy's role and href
/// (SACM23-XMI-029; ADR 0040 amendment 1). It gives no diagnostic, or, if
/// `template` marks an `xsi:type`, one SACM-XMI-010 at the mark affecting the
/// holder (an Error strict, a Warning tolerant); never SACM-XMI-005, and
/// nothing is kept as preserved content. `row` names the row in the messages.
fn assert_held_as_proxy(row: &str, what: &str, template: &str, held: (&str, &str, &str)) {
    let (holder, role, href) = held;
    let (input, marks) = marked(template);
    assert!(
        marks.len() <= 1,
        "{row}: setup: {what} has more than one mark"
    );
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!("{row}: {what}: a {mode:?} read refused it: {error:?} ({error})"),
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code().as_str(),
                    found.severity(),
                    found.location(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        let expected: Vec<_> = marks
            .iter()
            .map(|mark| {
                (
                    "SACM-XMI-010",
                    severity,
                    Some(*mark),
                    vec![ElementId::from(holder)],
                )
            })
            .collect();
        assert_eq!(
            found, expected,
            "{row}: {what}: the diagnostics of a {mode:?} read"
        );
        let doc = output.document();
        assert_eq!(
            (
                doc.get(holder).map(|holder| holder
                    .proxies()
                    .iter()
                    .map(|proxy| (proxy.role(), proxy.href().as_str()))
                    .collect::<Vec<_>>()),
                doc.has_preserved_content()
            ),
            (Some(vec![(role, href)]), false),
            "{row}: {what}: a {mode:?} read does not hold it as a proxy of {holder:?}"
        );
    }
}

#[test]
fn lib_api_011_a_role_element_with_an_unqualified_href_is_a_proxy_and_no_refusal() {
    // A role element carrying an unqualified `href` is the reference form of
    // a composite held in another document (XMI 2.5.1 §7.10.1, §7.10.2), a
    // proxy, whether it is typed or untyped and whether its role's declared
    // type is abstract or concrete: SACM23-XMI-029 reads it since slice
    // 0073b, so it is no refusal any more (it was `Unsupported` naming that
    // row; the maintainer, 2026-11-16; split from SACM23-XMI-027 by slice
    // 0068, ADR 0040, and read under its amendment 1). Its type, by
    // `xmi:type` or `xsi:type` (with its SACM-XMI-010, at the attribute), is
    // held as the href's type.
    // A qualified `xlink:href` or `xmi:href` is not `href` (XMI.xsd
    // LinkAttribs) and makes no proxy: the next test pins that it is
    // preserved, and the one after it that an untyped element carrying it
    // under an abstract role is still SACM-XMI-005.
    const API_011: &str = "LIB-API-011";
    let cases = [
        (
            "a typed argumentElement href to another document in an ArgumentPackage",
            ap("<argumentElement xmi:type=\"sacm:Claim\" href=\"o.xmi#c1\"/>"),
            ("p", "argumentElement", "sacm:Claim o.xmi#c1"),
        ),
        (
            "an xsi-typed argumentElement href to another document in an ArgumentPackage",
            ap(&format!(
                "<argumentElement {MARK}xsi:type=\"sacm:Claim\" href=\"o.xmi#c1\"/>"
            )),
            ("p", "argumentElement", "sacm:Claim o.xmi#c1"),
        ),
        (
            "an untyped argumentPackage href in an AssuranceCasePackage (a concrete role)",
            acp("<argumentPackage href=\"o.xmi#p1\"/>"),
            ("case", "argumentPackage", "o.xmi#p1"),
        ),
        (
            "a typed argumentPackage href in an AssuranceCasePackage",
            acp("<argumentPackage xmi:type=\"sacm:ArgumentPackage\" href=\"o.xmi#p1\"/>"),
            ("case", "argumentPackage", "sacm:ArgumentPackage o.xmi#p1"),
        ),
        (
            "an untyped argumentElement href in an ArgumentPackage",
            ap("<argumentElement href=\"o.xmi#c1\"/>"),
            ("p", "argumentElement", "o.xmi#c1"),
        ),
    ];
    for (what, template, held) in &cases {
        assert_held_as_proxy(API_011, what, template, *held);
    }
    // `href="#c1"` names an element of this document: since slice 0073a a
    // same-file proxy (SACM23-XMI-030), kept at its start tag, as `c1` is no
    // root; it named SACM23-XMI-029 until then.
    assert_proxy_of_no_root_at_mark(
        API_011,
        "a typed argumentElement href=\"#c1\" in an ArgumentPackage",
        &ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n  \
             {MARK}<argumentElement xmi:type=\"sacm:Claim\" href=\"#c1\"/>"
        )),
        "p",
    );
}

/// `root`, an [`ap`] or [`acp`] root, with `xmlns:xlink` declared on it.
/// The root's start tag is line 1, so the marks on line 3 do not move.
fn with_xlink(root: &str) -> String {
    root.replacen(
        " xmlns:xsi=",
        " xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:xsi=",
        1,
    )
}

#[test]
fn lib_api_011_a_role_element_with_a_qualified_href_is_preserved_not_refused() {
    // XMI 2.5.1 LinkAttribs (XMI.xsd) give a role element only an
    // unqualified `href`; a qualified `xlink:href` or `xmi:href` does not
    // make it a proxy, so it is not refused naming SACM23-XMI-029. On a
    // typed element, or one under a concrete role, it is an attribute this
    // release does not read: preserved on its element with one SACM-XMI-003
    // at its name, a Warning in tolerant mode and an Error in strict mode,
    // and written back.
    const API_011: &str = "LIB-API-011";
    let typed = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" ";
    let cases = [
        (
            "a typed argumentElement xlink:href in an ArgumentPackage",
            with_xlink(&ap(&format!("{typed}{MARK}xlink:href=\"o.xmi#c1\"/>"))),
            "c1",
            "xlink:href=\"o.xmi#c1\"",
        ),
        (
            "a typed argumentElement xmi:href in an ArgumentPackage",
            with_xlink(&ap(&format!("{typed}{MARK}xmi:href=\"o.xmi#c1\"/>"))),
            "c1",
            "xmi:href=\"o.xmi#c1\"",
        ),
        (
            "an untyped argumentPackage xlink:href in an AssuranceCasePackage",
            with_xlink(&acp(&format!(
                "<argumentPackage xmi:id=\"p1\" {MARK}xlink:href=\"o.xmi#p1\"/>"
            ))),
            "p1",
            "xlink:href=\"o.xmi#p1\"",
        ),
        (
            "an untyped argumentPackage xmi:href in an AssuranceCasePackage",
            with_xlink(&acp(&format!(
                "<argumentPackage xmi:id=\"p1\" {MARK}xmi:href=\"o.xmi#p1\"/>"
            ))),
            "p1",
            "xmi:href=\"o.xmi#p1\"",
        ),
    ];
    for (what, template, holder, attribute) in &cases {
        let (input, marks) = marked(template);
        assert_eq!(marks.len(), 1, "{API_011}: setup: {what} has not one mark");
        let mut documents = Vec::new();
        for (mode, severity) in [
            (Mode::Strict, Severity::Error),
            (Mode::Tolerant, Severity::Warning),
        ] {
            let output = match read(mode, &input) {
                Ok(output) => output,
                Err(error) => panic!(
                    "{API_011}: {what}: a {mode:?} read refused it instead of preserving it: \
                     {error:?} ({error})"
                ),
            };
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .filter(|found| found.code() == Code::XMI_UNKNOWN_CONTENT)
                .map(|found| {
                    (
                        found.severity(),
                        found.location(),
                        found.affected().to_vec(),
                    )
                })
                .collect();
            assert_eq!(
                found,
                [(severity, Some(marks[0]), vec![ElementId::from(*holder)])],
                "{API_011}: {what}: not one SACM-XMI-003 at the attribute's name affecting \
                 {holder:?} in {mode:?} mode: {:?}",
                output.diagnostics()
            );
            documents.push(output.into_document());
        }
        let written = match xmi::write(
            &documents[1],
            &WriteOptions::default().with_mode(Mode::Tolerant),
        ) {
            Ok(text) => text,
            Err(error) => panic!("{API_011}: {what}: a tolerant write refused it: {error:?}"),
        };
        assert!(
            written.contains(attribute),
            "{API_011}: {what}: the tolerant write lost {attribute}: {written}"
        );
    }
}

#[test]
fn lib_compat_010_a_qualified_href_does_not_make_a_reference_form() {
    // Only an unqualified `href` is the reference form of a composite (XMI
    // 2.5.1 §7.10.1; XMI.xsd LinkAttribs). An untyped argumentElement with a
    // qualified `xlink:href` or `xmi:href` is no proxy: it is an untyped
    // child under an abstract role, SACM-XMI-005 at its start tag.
    for (what, body) in [
        (
            "an untyped argumentElement xlink:href",
            format!("{MARK}<argumentElement xlink:href=\"o.xmi#c1\"/>"),
        ),
        (
            "an untyped argumentElement xmi:href",
            format!("{MARK}<argumentElement xmi:href=\"o.xmi#c1\"/>"),
        ),
    ] {
        assert_missing_type_at_mark(what, &with_xlink(&ap(&body)));
    }
}

#[test]
fn lib_api_011_a_role_element_with_xmi_idref_is_a_same_file_proxy() {
    // A role element carrying `xmi:idref` is the reference form of a
    // composite held elsewhere (XMI 2.5.1 §7.10.1), a proxy, whether it is
    // typed or untyped and whether its role's declared type is abstract or
    // concrete. Until slice 0073a it was refused naming SACM23-XMI-019 (the
    // maintainer, 2026-11-16); since, it is a same-file proxy
    // (SACM23-XMI-030): `c1` and `p1` name no root, so each is kept with
    // `SACM-XMI-003/proxy-of-no-root` at its start tag, in both modes, and
    // no refusal names SACM23-XMI-019.
    const API_011: &str = "LIB-API-011";
    let claim = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n  ";
    let cases = [
        (
            "a typed argumentElement xmi:idref in an ArgumentPackage",
            ap(&format!(
                "{claim}{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:idref=\"c1\"/>"
            )),
            "p",
        ),
        (
            "an untyped argumentPackage xmi:idref in an AssuranceCasePackage (a concrete role)",
            acp(&format!("{MARK}<argumentPackage xmi:idref=\"p1\"/>")),
            "case",
        ),
        (
            "a typed argumentPackage xmi:idref in an AssuranceCasePackage",
            acp(&format!(
                "{MARK}<argumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:idref=\"p1\"/>"
            )),
            "case",
        ),
        (
            "an untyped argumentElement xmi:idref in an ArgumentPackage",
            ap(&format!("{claim}{MARK}<argumentElement xmi:idref=\"c1\"/>")),
            "p",
        ),
    ];
    for (what, template, holder) in &cases {
        assert_proxy_of_no_root_at_mark(API_011, what, template, holder);
    }
}

#[test]
fn lib_compat_010_the_same_element_typed_as_a_concrete_admitted_class_reads_as_before() {
    for (what, body, kind) in [
        (
            "xmi:type Claim",
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>",
            ElementKind::Claim,
        ),
        (
            "xsi:type Claim",
            "<argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>",
            ElementKind::Claim,
        ),
        (
            "xmi:type ArgumentReasoning",
            "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"c1\"/>",
            ElementKind::ArgumentReasoning,
        ),
        (
            "xmi:type ArgumentPackage",
            "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"c1\"/>",
            ElementKind::ArgumentPackage,
        ),
    ] {
        let doc = read_ok(what, &ap(body));
        let Some(c1) = doc.get("c1") else {
            panic!("{ROW}: {what}: the element was not read");
        };
        assert_eq!(
            (c1.kind(), c1.parent().map(|parent| parent.id().as_str())),
            (kind, Some("p")),
            "{ROW}: {what}: not read as a {kind} in p"
        );
    }
}

#[test]
fn lib_compat_010_an_untyped_child_under_a_concrete_role_still_reads() {
    let doc = read_ok(
        "an untyped argumentPackage",
        &acp("<argumentPackage xmi:id=\"a\"><name content=\"A\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/></argumentPackage>\n  \
             <assuranceCasePackage xmi:id=\"sub\"/>"),
    );
    for (id, kind, parent) in [
        ("a", ElementKind::ArgumentPackage, "case"),
        ("c1", ElementKind::Claim, "a"),
        ("sub", ElementKind::AssuranceCasePackage, "case"),
    ] {
        let Some(found) = doc.get(id) else {
            panic!("{ROW}: the untyped child {id:?} was not read");
        };
        assert_eq!(
            (found.kind(), found.parent().map(|p| p.id().as_str())),
            (kind, Some(parent)),
            "{ROW}: the untyped child {id:?} is not read as a {kind} in {parent}"
        );
    }
    // A UtilityElement role is concrete too.
    let doc = read_ok(
        "an untyped description",
        &ap("<description xmi:id=\"d\"/><note xmi:id=\"n\"/>"),
    );
    assert_eq!(
        (
            doc.get("d").map(SacmElement::kind),
            doc.get("n").map(SacmElement::kind)
        ),
        (Some(ElementKind::Description), Some(ElementKind::Note)),
        "{ROW}: untyped utility elements are not read"
    );
}

#[test]
fn lib_compat_010_the_catalog_row_of_sacm_xmi_005_is_live_and_an_error_in_strict_mode() {
    assert_eq!(
        Code::XMI_MISSING_TYPE.as_str(),
        "SACM-XMI-005",
        "{ROW}: Code::XMI_MISSING_TYPE is not SACM-XMI-005"
    );
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let rows: Vec<&str> = catalog
        .lines()
        .filter(|line| line.trim_start().starts_with("| SACM-XMI-005 |"))
        .collect();
    let [row] = rows.as_slice() else {
        panic!(
            "{ROW}: docs/sacm/diagnostics.md has {} rows for SACM-XMI-005, not one",
            rows.len()
        );
    };
    let cells: Vec<&str> = row.split('|').map(str::trim).collect();
    assert_eq!(
        cells.get(2).copied(),
        Some("Error (strict) / Warning (tolerant read)"),
        "{ROW}: the severity of SACM-XMI-005 is not an Error in strict mode and a Warning \
         in a tolerant read: {row}"
    );
    for needle in ["`Code::XMI_MISSING_TYPE`", "LIB-COMPAT-010"] {
        assert!(
            row.contains(needle),
            "{ROW}: the catalog row of SACM-XMI-005 does not name {needle}: {row}"
        );
    }
}
