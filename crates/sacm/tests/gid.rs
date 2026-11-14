//! Tests for `gid`, the optional global identifier of every `SACMElement`
//! (SACM23-BASE-006; SACM 2.3 §8.2; XMI 2.5.1 §7.8.4, rule 2h; ADR 0012,
//! ADR 0021 amendment 1, ADR 0025).
//!
//! `gid` is a String held exactly: absent, `""` and any other string differ,
//! and nothing is trimmed. It is read in both modes from the unqualified
//! attribute (after XML attribute-value normalisation) or from a `gid` value
//! element (SACM23-XMI-014), and given twice it is `RepeatedValue`. It is not
//! an `xmi:id`: no id space, no syntax check, no generation. `xmi::write`
//! writes it as an attribute directly after `xmi:id`, escaped. It is compared
//! as `Difference::Gid`, after `Parent`.
//!
//! Not checked here: that a character XML 1.0 cannot represent in a gid fails
//! the write with `WriteAttribute::Gid`. No public API sets a gid (LIB-EDIT-008
//! will), and an XML input cannot hold such a character, so no test document
//! can carry one; `WriteAttribute::Gid`'s `Display` is checked instead.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading.

use std::path::Path;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, SourceLocation};
use sacm::edit::{self, CreateClaim};
use sacm::model::{Document, ElementId, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteOptions};

const ROW: &str = "SACM23-BASE-006";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a start tag, an attribute or a child is
/// expected. Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The valid fixture of this row.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/gid-valid.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------

/// Where a gid is placed: what, and the input given the start-tag attributes
/// and the leading content of the element `x` that carries it.
type Holder = fn(&str, &str) -> String;

/// The elements a gid is tested on: an AssuranceCasePackage root, an
/// ArgumentPackage root, a Claim, and each of the four `UtilityElement` kinds.
fn holders() -> [(&'static str, Holder); 7] {
    [
        (
            "an AssuranceCasePackage root",
            |attributes: &str, content: &str| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"x\"{attributes}>{content}\
                 <name content=\"X\"/></sacm:AssuranceCasePackage>"
                )
            },
        ),
        (
            "an ArgumentPackage root",
            |attributes: &str, content: &str| {
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"x\"{attributes}>{content}\
                 <name content=\"X\"/></sacm:ArgumentPackage>"
                )
            },
        ),
        ("a Claim", |attributes: &str, content: &str| {
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"{attributes}>{content}\
                 <name content=\"X\"/></argumentElement>"
            ))
        }),
        ("a Description", |attributes: &str, content: &str| {
            in_ap(&format!(
                "<description xmi:id=\"x\"{attributes}>{content}\
                 <content><value content=\"d\"/></content></description>"
            ))
        }),
        (
            "an ImplementationConstraint",
            |attributes: &str, content: &str| {
                in_ap(&format!(
                    "<implementationConstraint xmi:id=\"x\"{attributes}>{content}</implementationConstraint>"
                ))
            },
        ),
        ("a Note", |attributes: &str, content: &str| {
            in_ap(&format!("<note xmi:id=\"x\"{attributes}>{content}</note>"))
        }),
        ("a TaggedValue", |attributes: &str, content: &str| {
            in_ap(&format!(
                "<taggedValue xmi:id=\"x\"{attributes}>{content}\
                 <key><value content=\"k\"/></key></taggedValue>"
            ))
        }),
    ]
}

/// An ArgumentPackage root `p` named "P", holding `body` from line 3.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// A Claim `id` named "C" with `attributes` and holding `content`.
fn claim(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"C\"/>{content}</argumentElement>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, CRLF or a lone CR, and columns count characters.
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

// --- Reading and writing -------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to equal documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
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

/// Reads `input` in both modes, which the test expects to be refused the same
/// in both, and returns the refusal.
fn refused(what: &str, input: &str) -> ReadError {
    let mut errors = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => panic!(
                "{ROW}: {what}: a {mode:?} read returned a document of {} elements for {input:?}",
                output.document().element_count()
            ),
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(
        errors[0], errors[1],
        "{ROW}: {what}: the modes refuse differently"
    );
    errors.swap_remove(0)
}

/// Writes `doc` tolerantly: a missing `TaggedValue` key or name is a validation
/// Error, and the gid is what is tested, not the save mode.
fn write(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: the write refused the document: {error:?}"),
    }
}

/// The element `id` of `doc`, which must exist.
fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

fn gid_of(doc: &Document, id: &str) -> Option<String> {
    element(doc, id).gid().map(str::to_owned)
}

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

// --- Item 1: the forms, on every kind -----------------------------------------------

/// The gid forms: what, the start-tag attributes, the leading content, and
/// the value read.
fn forms() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
)> {
    vec![
        ("no gid", "", "", None),
        ("gid=\"g\"", " gid=\"g\"", "", Some("g")),
        ("gid=\"\"", " gid=\"\"", "", Some("")),
        ("<gid>g</gid>", "", "<gid>g</gid>", Some("g")),
        ("<gid/>", "", "<gid/>", Some("")),
        ("<gid></gid>", "", "<gid></gid>", Some("")),
        (
            "<gid> CDATA",
            "",
            "<gid><![CDATA[a<b]]>&amp;c</gid>",
            Some("a<b&c"),
        ),
        (
            "<gid> character references",
            "",
            "<gid>&#x41;&#66;&lt;&gt;&quot;&apos;</gid>",
            Some("AB<>\"'"),
        ),
        (
            "<gid> with a comment",
            "",
            "<gid>a<!-- c -->b</gid>",
            Some("ab"),
        ),
        ("gid=\"  g \"", " gid=\"  g \"", "", Some("  g ")),
        ("<gid> g </gid>", "", "<gid> g </gid>", Some(" g ")),
        (
            "a gid that is not an NCName",
            " gid=\"1 a:b\"",
            "",
            Some("1 a:b"),
        ),
    ]
}

#[test]
fn sacm23_base_006_every_gid_form_reads_exactly_on_every_kind_and_round_trips() {
    for (holder, input_of) in holders() {
        for (form, attributes, content, expected) in forms() {
            let what = format!("{form} on {holder}");
            let doc = read_ok(&what, &input_of(attributes, content));
            assert_eq!(
                element(&doc, "x").gid(),
                expected,
                "{ROW}: {what}: the gid is not read exactly"
            );
            let text = write(&what, &doc);
            let back = read_ok(&format!("{what}, written"), &text);
            assert_eq!(
                (semantic_compare(&doc, &back), gid_of(&back, "x")),
                (Vec::new(), expected.map(str::to_owned)),
                "{ROW}: {what}: the gid does not read back equal\n{text}"
            );
            assert_eq!(back, doc, "{ROW}: {what}: the output reads back !=");
            assert_eq!(
                write(&what, &back),
                text,
                "{ROW}: {what}: writing what was read back gives other bytes"
            );
        }
    }
}

#[test]
fn sacm23_base_006_the_element_form_may_appear_anywhere_among_the_siblings() {
    let attribute = read_ok(
        "the attribute form",
        &in_ap(&claim("c1", " gid=\"g\"", "<description xmi:id=\"d\"/>")),
    );
    for (what, content) in [
        ("after the name", "<gid>g</gid><description xmi:id=\"d\"/>"),
        ("after a child", "<description xmi:id=\"d\"/><gid>g</gid>"),
    ] {
        let doc = read_ok(what, &in_ap(&claim("c1", "", content)));
        assert_eq!(
            semantic_compare(&doc, &attribute),
            [],
            "{ROW}: {what}: the element form does not read equal to the attribute form"
        );
    }
    let before_name = read_ok(
        "before the name",
        &in_ap(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><gid>g</gid>\
             <name content=\"C\"/><description xmi:id=\"d\"/></argumentElement>",
        ),
    );
    assert_eq!(
        semantic_compare(&before_name, &attribute),
        [],
        "{ROW}: before the name: the element form does not read equal"
    );
}

#[test]
fn sacm23_base_006_the_fixture_reads_with_its_gids_and_round_trips() {
    let bytes = match std::fs::read(Path::new(FIXTURE)) {
        Ok(bytes) => bytes,
        Err(error) => panic!("{ROW}: cannot read {FIXTURE}: {error}"),
    };
    let doc = read_ok("the fixture", &String::from_utf8_lossy(&bytes));
    let gids: Vec<_> = ["case", "arguments", "c1", "n1"]
        .into_iter()
        .map(|id| gid_of(&doc, id))
        .collect();
    assert_eq!(
        gids,
        [
            Some("urn:example:case".to_owned()),
            Some(" arguments-1 ".to_owned()),
            Some(String::new()),
            Some("c1".to_owned())
        ],
        "{ROW}: the fixture's gids"
    );
    let text = write("the fixture", &doc);
    assert_eq!(
        read_ok("the fixture, written", &text),
        doc,
        "{ROW}: the fixture does not read back equal"
    );
}

// --- Item 2: normalisation and escaping --------------------------------------------

#[test]
fn sacm23_base_006_a_gid_attribute_is_read_after_attribute_value_normalisation() {
    for (what, attribute, expected) in [
        ("a character reference to LF", "a&#10;b", "a\nb"),
        (
            "a character reference to TAB and CR",
            "a&#9;b&#13;",
            "a\tb\r",
        ),
        ("a literal LF", "a\nb", "a b"),
        ("a literal TAB", "a\tb", "a b"),
        ("a literal CRLF", "a\r\nb", "a b"),
        ("entity references", "&amp;&lt;&gt;&quot;&apos;", "&<>\"'"),
    ] {
        let doc = read_ok(
            what,
            &in_ap(&claim("c1", &format!(" gid=\"{attribute}\""), "")),
        );
        assert_eq!(
            gid_of(&doc, "c1").as_deref(),
            Some(expected),
            "{ROW}: {what}: not read as {expected:?}"
        );
    }
}

#[test]
fn sacm23_base_006_the_writer_escapes_markup_tab_lf_and_cr_and_the_gid_reads_back_equal() {
    let doc = read_ok(
        "every escaped character",
        &in_ap(&claim("c1", "", "<gid>&amp;&lt;&gt;\"\t\n&#13;x</gid>")),
    );
    assert_eq!(
        gid_of(&doc, "c1").as_deref(),
        Some("&<>\"\t\n\rx"),
        "{ROW}: setup: the element form is not read exactly"
    );
    let text = write("every escaped character", &doc);
    let line = line_with(&text, "c1");
    assert!(
        line.contains(" gid=\"&amp;&lt;&gt;&quot;&#9;&#10;&#13;x\""),
        "{ROW}: the gid is not written escaped: {line}"
    );
    assert_eq!(
        read_ok("written", &text),
        doc,
        "{ROW}: the escaped gid does not read back equal"
    );
    let shown = WriteAttribute::Gid.to_string();
    assert!(
        shown.contains("gid"),
        "{ROW}: WriteAttribute::Gid displays as {shown:?}"
    );
}

// --- Item 3: repeats -----------------------------------------------------------------

#[test]
fn sacm23_base_006_a_gid_given_twice_is_repeated_value_with_every_location() {
    for (holder, input_of) in holders() {
        for (what, attributes, content) in [
            (
                "the attribute and the element",
                format!(" {MARK}gid=\"g\""),
                format!("{MARK}<gid>g</gid>"),
            ),
            (
                "the attribute and a different element",
                format!(" {MARK}gid=\"\""),
                format!("{MARK}<gid>h</gid>"),
            ),
            (
                "two elements",
                String::new(),
                format!("{MARK}<gid>g</gid>{MARK}<gid>g</gid>"),
            ),
        ] {
            let what = format!("{what} on {holder}");
            let (input, marks) = marked(&input_of(&attributes, &content));
            let error = refused(&what, &input);
            assert!(
                matches!(
                    &error,
                    ReadError::RepeatedValue { element, property, .. }
                        if element.as_str() == "x" && property == "gid"
                ),
                "{ROW}: {what}: not RepeatedValue {{ x, gid }}: {error:?}"
            );
            assert_eq!(
                (error.code(), error.locations(), error.location()),
                (
                    Some(Code::MULTIPLICITY_VIOLATION),
                    marks.as_slice(),
                    marks.get(1).copied()
                ),
                "{ROW}: {what}: not SACM-MULT-001 at every occurrence"
            );
        }
    }
}

// --- Item 4: not an id -------------------------------------------------------------

#[test]
fn sacm23_base_006_a_gid_is_not_an_id_and_two_elements_may_share_one() {
    let doc = read_ok(
        "gids equal to ids and to each other",
        &in_ap(&format!(
            "{}{}{}",
            claim("c1", " gid=\"c1\"", ""),
            claim("c2", " gid=\"p\"", ""),
            claim("c3", " gid=\"p\"", "")
        )),
    );
    assert_eq!(
        ["c1", "c2", "c3"].map(|id| gid_of(&doc, id)),
        [
            Some("c1".to_owned()),
            Some("p".to_owned()),
            Some("p".to_owned())
        ],
        "{ROW}: the gids are not all kept"
    );
    // A gid is in no id space: the generator does not skip it, an edit may
    // use it as an id, and `get` does not find it.
    for mode in MODES {
        let output = match read(
            mode,
            &in_ap(
                "<argumentElement xmi:type=\"sacm:Claim\" gid=\"claim_1\"><name content=\"C\"/>\
                 </argumentElement>",
            ),
        ) {
            Ok(output) => output,
            Err(error) => panic!("{ROW}: an id-less Claim with a gid ({mode:?}): {error:?}"),
        };
        assert_eq!(
            output.document().get("claim_1").and_then(SacmElement::gid),
            Some("claim_1"),
            "{ROW}: ({mode:?}) the id-less Claim is not claim_1: a gid took an id"
        );
    }
    let mut doc = read_ok("a gid g1", &in_ap(&claim("c1", " gid=\"g1\"", "")));
    assert!(doc.get("g1").is_none(), "{ROW}: get finds a gid");
    let created = edit::apply(
        &mut doc,
        CreateClaim::new("p", LangString::new("C")).with_id("g1"),
    );
    assert!(
        created.is_ok_and(|m| m.created_ids() == [ElementId::from("g1")]),
        "{ROW}: an id equal to a gid is refused"
    );
}

#[test]
fn sacm23_base_006_a_gid_has_no_syntax_check_and_is_never_trimmed() {
    for gid in [" g ", "1 a:b", "", "\u{b7}x", " "] {
        let what = format!("gid {gid:?}");
        let doc = read_ok(&what, &in_ap(&claim("c1", &format!(" gid=\"{gid}\""), "")));
        assert_eq!(
            gid_of(&doc, "c1").as_deref(),
            Some(gid),
            "{ROW}: {what}: not held exactly"
        );
        let found = validation::validate(&doc);
        assert!(
            found.iter().all(|f| f.code() != Code::ID_INVALID),
            "{ROW}: {what}: validate checks the gid's syntax: {found:?}"
        );
    }
}

// --- Item 5: refusals ---------------------------------------------------------------

#[test]
fn sacm23_base_006_a_gid_in_the_wrong_place_or_form_is_unsupported_at_that_construct() {
    let description =
        |body: &str| in_ap(&format!("<description xmi:id=\"d\">{body}</description>"));
    let cases = [
        (
            "sacm:gid as an attribute",
            in_ap(&claim("c1", &format!(" {MARK}sacm:gid=\"g\""), "")),
        ),
        (
            "sacm:gid as an element",
            in_ap(&claim("c1", "", &format!("{MARK}<sacm:gid>g</sacm:gid>"))),
        ),
        (
            "gid on name",
            in_ap(&claim(
                "c1",
                "",
                &format!("<name content=\"N\" {MARK}gid=\"g\"/>"),
            ))
            .replacen("<name content=\"C\"/>", "", 1),
        ),
        (
            "a gid element in name",
            in_ap(&claim(
                "c1",
                "",
                &format!("<name content=\"N\">{MARK}<gid>g</gid></name>"),
            ))
            .replacen("<name content=\"C\"/>", "", 1),
        ),
        (
            "gid on content",
            description(&format!("<content {MARK}gid=\"g\"/>")),
        ),
        (
            "a gid element in content",
            description(&format!("<content>{MARK}<gid>g</gid></content>")),
        ),
        (
            "gid on key",
            in_ap(&format!(
                "<taggedValue xmi:id=\"t\"><key {MARK}gid=\"g\"/></taggedValue>"
            )),
        ),
        (
            "gid on value",
            description(&format!(
                "<content><value {MARK}gid=\"g\" content=\"x\"/></content>"
            )),
        ),
        (
            "a gid element in value",
            description(&format!(
                "<content><value>{MARK}<gid>g</gid></value></content>"
            )),
        ),
        (
            "an xmi:id on a gid element",
            in_ap(&claim(
                "c1",
                "",
                &format!("<gid {MARK}xmi:id=\"x\">g</gid>"),
            )),
        ),
        (
            "an element in a gid element",
            in_ap(&claim("c1", "", &format!("<gid>g{MARK}<b/></gid>"))),
        ),
        (
            "xsi:nil on a gid element",
            in_ap(&claim("c1", "", &format!("<gid {MARK}xsi:nil=\"true\"/>"))),
        ),
    ];
    for (what, template) in &cases {
        let (input, marks) = marked(template);
        assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
        let error = refused(what, &input);
        assert!(
            matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
            "{ROW}: {what}: not Unsupported: {error:?}"
        );
        assert_eq!(
            error.location(),
            Some(marks[0]),
            "{ROW}: {what}: not refused at the construct: {error}"
        );
    }
}

// --- Item 6: comparison ---------------------------------------------------------------

#[test]
fn sacm23_base_006_absent_empty_and_other_gids_are_each_one_gid_difference() {
    let with = |what: &str, attributes: &str| read_ok(what, &in_ap(&claim("c1", attributes, "")));
    let absent = with("absent", "");
    let empty = with("empty", " gid=\"\"");
    let g = with("g", " gid=\"g\"");
    let gid = |value: &str| Some(value.to_owned());
    let cases = [
        ("absent and empty", &absent, &empty, None, gid("")),
        ("empty and g", &empty, &g, gid(""), gid("g")),
        ("g and absent", &g, &absent, gid("g"), None),
    ];
    for (what, left, right, left_gid, right_gid) in cases {
        assert_ne!(left, right, "{ROW}: {what}: the documents are ==");
        let expected = Difference::Gid {
            id: ElementId::from("c1"),
            left: left_gid,
            right: right_gid,
        };
        assert_eq!(
            semantic_compare(left, right),
            std::slice::from_ref(&expected),
            "{ROW}: {what}: not one Gid difference"
        );
        assert_eq!(
            expected.element_id(),
            Some(&ElementId::from("c1")),
            "{ROW}: {what}: element_id()"
        );
        let shown = expected.to_string();
        assert!(
            !shown.contains('\n') && shown.contains("\"c1\"") && shown.contains("gid"),
            "{ROW}: {what}: Display {shown:?} is not one line naming \"c1\" and gid"
        );
    }
    let absent_shown = Difference::Gid {
        id: ElementId::from("c1"),
        left: None,
        right: gid("g"),
    }
    .to_string();
    let empty_shown = Difference::Gid {
        id: ElementId::from("c1"),
        left: gid(""),
        right: gid("g"),
    }
    .to_string();
    assert_ne!(
        absent_shown, empty_shown,
        "{ROW}: an absent and an empty gid display the same"
    );
}

#[test]
fn sacm23_base_006_a_gid_difference_comes_after_parent_and_before_the_name_and_content() {
    // c1 moves from a1 to a2, and its gid and name differ too.
    let doc = |what: &str, holder: &str, gid: &str, name: &str| {
        let c1 = format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{gid}><name content=\"{name}\"/>\
             </argumentElement>"
        );
        let (a1, a2) = if holder == "a1" {
            (c1.as_str(), "")
        } else {
            ("", c1.as_str())
        };
        read_ok(
            what,
            &in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"a1\"><name content=\"A\"/>{a1}</argumentElement>\
                 <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"a2\"><name content=\"A\"/>{a2}</argumentElement>"
            )),
        )
    };
    let left = doc("left", "a1", "", "C");
    let right = doc("right", "a2", " gid=\"g\"", "D");
    let differences: Vec<Difference> = semantic_compare(&left, &right)
        .into_iter()
        .filter(|d| d.element_id() == Some(&ElementId::from("c1")))
        .collect();
    assert!(
        matches!(
            differences.as_slice(),
            [
                Difference::Parent { .. },
                Difference::Gid { .. },
                Difference::Name { .. }
            ]
        ),
        "{ROW}: c1's differences are not Parent, Gid, Name: {differences:?}"
    );
    // On a UtilityElement: Gid before Content.
    let note = |what: &str, body: &str| read_ok(what, &in_ap(body));
    let left = note(
        "a note",
        "<note xmi:id=\"n\"><content><value content=\"a\"/></content></note>",
    );
    let right = note(
        "another note",
        "<note xmi:id=\"n\" gid=\"g\"><content><value content=\"b\"/></content></note>",
    );
    let differences = semantic_compare(&left, &right);
    assert!(
        matches!(
            differences.as_slice(),
            [Difference::Gid { .. }, Difference::Content { .. }]
        ),
        "{ROW}: a Note's differences are not Gid, Content: {differences:?}"
    );
}

// --- Write order -------------------------------------------------------------------------

#[test]
fn sacm23_base_006_a_gid_is_written_directly_after_xmi_id_and_before_the_class_attributes() {
    let doc = read_ok(
        "gids on every kind",
        &format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\" gid=\"g0\"><name content=\"Case\"/>\
             <assuranceCasePackage gid=\"g1\" xmi:id=\"sub\"><name content=\"S\"/></assuranceCasePackage>\
             <argumentPackage xmi:id=\"ap\"><gid>g2</gid><name content=\"A\"/>\
             <argumentElement assertionDeclaration=\"needsSupport\" gid=\"g3\" xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name content=\"C\"/><note gid=\"\" xmi:id=\"n\"/></argumentElement>\
             <argumentElement gid=\"g4\" xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner\"><name content=\"I\"/></argumentElement>\
             </argumentPackage></sacm:AssuranceCasePackage>"
        ),
    );
    let text = write("gids on every kind", &doc);
    let expected = [
        ("case", "xmi:id=\"case\" gid=\"g0\">"),
        ("sub", "<assuranceCasePackage xmi:id=\"sub\" gid=\"g1\">"),
        ("ap", "<argumentPackage xmi:id=\"ap\" gid=\"g2\">"),
        (
            "c1",
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" gid=\"g3\" \
             assertionDeclaration=\"needsSupport\">",
        ),
        ("n", "<note xmi:id=\"n\" gid=\"\" />"),
        (
            "inner",
            "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner\" gid=\"g4\">",
        ),
    ];
    for (id, tail) in expected {
        let line = line_with(&text, id);
        assert!(
            line.ends_with(tail),
            "{ROW}: the start tag of {id:?} does not end with {tail:?}: {line}"
        );
    }
    assert_eq!(
        read_ok("written", &text),
        doc,
        "{ROW}: the written gids do not read back equal"
    );
    // An absent gid writes nothing.
    let plain = read_ok("no gid", &in_ap(&claim("c1", "", "")));
    let text = write("no gid", &plain);
    assert!(
        !text.contains("gid"),
        "{ROW}: an absent gid is written:\n{text}"
    );
}
