//! Tests for the `MultiLangString` rules of `validation::validate`
//! (SACM23-VAL-010; SACM 2.3 §8.5 `value:LangString[1..*]` and "their +lang
//! must be unique", §8.12 `key:MultiLangString[1]`; D-SEV; ADR 0012,
//! ADR 0025).
//!
//! For each present `UtilityElement` `content` and `TaggedValue` `key`, `validate`
//! reports one SACM-MULT-001 Error when it has no values, and one for each
//! language two or more of its values share; `lang` is compared exactly. A
//! `TaggedValue` with no `key` is one more. An absent `content` gives no
//! finding. Every diagnostic has the `UtilityElement` as its only affected id,
//! no location, requirement SACM23-VAL-010 and clause `8.5` (`8.12` for a
//! missing key). A strict save refuses; a tolerant save writes as held.
//!
//! Documents are read from inline XMI; every `ModelElement` in them is named,
//! so no other rule reports anything. The message is checked only for what it
//! must name (the role and the repeated language), never word for word.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::model::{Document, ElementId, LangString, MultiLangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "SACM23-VAL-010";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The four utility roles.
const ROLES: [&str; 4] = [
    "description",
    "implementationConstraint",
    "note",
    "taggedValue",
];

/// A key every `TaggedValue` below has unless the test says otherwise.
const KEY: &str = "<key><value content=\"k\"/></key>";

// --- Inputs and helpers ------------------------------------------------------------

/// A Claim `c1` named "C" holding `body`, in an ArgumentPackage root `p`
/// named "P".
fn in_claim(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/>{body}\
         </argumentElement></sacm:ArgumentPackage>"
    )
}

/// A `UtilityElement` `u` of `role` whose `content` holds `values`; a
/// `TaggedValue` also gets [`KEY`].
fn utility(role: &str, values: &str) -> String {
    let key = if role == "taggedValue" { KEY } else { "" };
    format!("<{role} xmi:id=\"u\">{key}<content>{values}</content></{role}>")
}

/// A Description `d` whose content holds `values`.
fn description(values: &str) -> String {
    in_claim(&utility("description", values).replace("\"u\"", "\"d\""))
}

/// Reads `input`, which the test expects to read in tolerant mode.
fn read(what: &str, input: &str) -> Document {
    match xmi::read(
        input.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: setup: refused: {error:?} ({error})"),
    }
}

/// The findings of `validate` on `doc`, which must be only this row's.
fn findings(what: &str, doc: &Document) -> Vec<Diagnostic> {
    let found = validation::validate(doc);
    for finding in &found {
        assert_eq!(
            finding.requirement_id(),
            ROW,
            "{ROW}: {what}: setup: another rule reports {finding:?}"
        );
    }
    found
}

/// Asserts the fields every SACM23-VAL-010 diagnostic has: an Error
/// SACM-MULT-001 on `affected` only, no location, the clause, and a message
/// naming each of `named`.
fn assert_finding(what: &str, found: &Diagnostic, affected: &str, clause: &str, named: &[&str]) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::MULTIPLICITY_VIOLATION, Severity::Error, ROW),
        "{ROW}: {what}: not an Error SACM-MULT-001 of this row"
    );
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        ([ElementId::from(affected)].as_slice(), None, Some(clause)),
        "{ROW}: {what}: the affected id, location or clause"
    );
    let message = found.message();
    for part in named {
        assert!(
            message.contains(part),
            "{ROW}: {what}: the message {message:?} does not name {part:?}"
        );
    }
}

/// Validates `input`, which must give exactly one finding, and returns it.
fn one_finding(what: &str, input: &str) -> Diagnostic {
    let found = findings(what, &read(what, input));
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    finding.clone()
}

// --- Item 1: empty MultiLangStrings and a missing key -------------------------------

#[test]
fn sacm23_val_010_an_empty_content_on_each_utility_kind_is_one_error_naming_the_element_and_content()
 {
    for role in ROLES {
        let what = format!("<content/> on a {role}");
        let finding = one_finding(&what, &in_claim(&utility(role, "")));
        assert_finding(&what, &finding, "u", "8.5", &["content"]);
    }
    // The same for `<content></content>` and a content of only white space.
    for form in ["<content></content>", "<content> \n </content>"] {
        let input = in_claim(&format!("<note xmi:id=\"n\">{form}</note>"));
        let finding = one_finding(form, &input);
        assert_finding(form, &finding, "n", "8.5", &["content"]);
    }
}

#[test]
fn sacm23_val_010_an_empty_key_is_one_error_naming_key_and_a_missing_key_one_with_clause_8_12() {
    let content = "<content><value content=\"v\"/></content>";
    let empty = one_finding(
        "<key/>",
        &in_claim(&format!(
            "<taggedValue xmi:id=\"t\"><key/>{content}</taggedValue>"
        )),
    );
    assert_finding("<key/>", &empty, "t", "8.5", &["key"]);
    let missing = one_finding(
        "no key",
        &in_claim(&format!(
            "<taggedValue xmi:id=\"t\">{content}</taggedValue>"
        )),
    );
    assert_finding("no key", &missing, "t", "8.12", &["key"]);
    // No key and no content: only the key.
    let bare = one_finding(
        "a bare TaggedValue",
        &in_claim("<taggedValue xmi:id=\"t\"/>"),
    );
    assert_finding("a bare TaggedValue", &bare, "t", "8.12", &["key"]);
    // An empty key and an empty content: two findings on the TaggedValue.
    let doc = read(
        "both empty",
        &in_claim("<taggedValue xmi:id=\"t\"><key/><content/></taggedValue>"),
    );
    let found = findings("both empty", &doc);
    assert_eq!(found.len(), 2, "{ROW}: both empty: {found:?}");
    for finding in &found {
        assert_finding("both empty", finding, "t", "8.5", &[]);
    }
    assert!(
        found.iter().any(|f| f.message().contains("key"))
            && found.iter().any(|f| f.message().contains("content")),
        "{ROW}: both empty: the two findings do not name key and content: {found:?}"
    );
}

// --- Item 2: repeated languages ----------------------------------------------------

#[test]
fn sacm23_val_010_a_language_two_or_more_values_share_is_one_error_naming_it() {
    let cases = [
        (
            "[en:a, en:b]",
            "<value lang=\"en\" content=\"a\"/><value lang=\"en\" content=\"b\"/>",
            "en",
        ),
        (
            "[en, en, en]",
            "<value lang=\"en\" content=\"a\"/><value lang=\"en\" content=\"a\"/>\
             <value lang=\"en\" content=\"c\"/>",
            "en",
        ),
        (
            "[sv-FI:a, da:b, sv-FI:c]",
            "<value lang=\"sv-FI\" content=\"a\"/><value lang=\"da\" content=\"b\"/>\
             <value lang=\"sv-FI\" content=\"c\"/>",
            "sv-FI",
        ),
        (
            "[no lang, no lang]",
            "<value content=\"a\"/><value content=\"b\"/>",
            "",
        ),
        (
            "[lang=\"\", lang=\"\"]",
            "<value lang=\"\" content=\"a\"/><value lang=\"\"/>",
            "",
        ),
    ];
    for (what, values, language) in cases {
        let finding = one_finding(what, &description(values));
        let named: &[&str] = if language.is_empty() {
            &["content"]
        } else {
            &["content", language]
        };
        assert_finding(what, &finding, "d", "8.5", named);
    }
    // The absent language and the empty one are named differently.
    let absent = one_finding(
        "absent",
        &description("<value content=\"a\"/><value content=\"b\"/>"),
    );
    let empty = one_finding(
        "empty",
        &description("<value lang=\"\" content=\"a\"/><value lang=\"\" content=\"b\"/>"),
    );
    assert_ne!(
        absent.message(),
        empty.message(),
        "{ROW}: a repeated absent lang and a repeated empty lang give the same message"
    );
    // A key's repeat names the key.
    let key = one_finding(
        "a repeated key language",
        &in_claim(
            "<taggedValue xmi:id=\"t\"><key><value lang=\"sv-FI\" content=\"a\"/>\
             <value lang=\"sv-FI\" content=\"b\"/></key><content><value content=\"v\"/></content></taggedValue>",
        ),
    );
    assert_finding(
        "a repeated key language",
        &key,
        "t",
        "8.5",
        &["key", "sv-FI"],
    );
}

#[test]
fn sacm23_val_010_each_repeated_language_is_its_own_error() {
    let doc = read(
        "[sv-FI, nb-NO, sv-FI, nb-NO, en]",
        &description(
            "<value lang=\"sv-FI\" content=\"a\"/><value lang=\"nb-NO\" content=\"b\"/>\
             <value lang=\"sv-FI\" content=\"c\"/><value lang=\"nb-NO\" content=\"d\"/>\
             <value lang=\"en\" content=\"e\"/>",
        ),
    );
    let found = findings("two repeated languages", &doc);
    assert_eq!(
        found.len(),
        2,
        "{ROW}: two repeated languages are not two findings: {found:?}"
    );
    for finding in &found {
        assert_finding("two repeated languages", finding, "d", "8.5", &["content"]);
    }
    assert!(
        found.iter().any(|f| f.message().contains("sv-FI"))
            && found.iter().any(|f| f.message().contains("nb-NO")),
        "{ROW}: the findings do not name sv-FI and nb-NO: {found:?}"
    );
}

// --- Item 3: no finding -------------------------------------------------------------

#[test]
fn sacm23_val_010_distinct_languages_compared_exactly_and_an_absent_content_give_no_finding() {
    let cases = [
        (
            "[no lang, lang=\"\"]",
            description("<value content=\"a\"/><value lang=\"\" content=\"b\"/>"),
        ),
        (
            "[en, EN]",
            description("<value lang=\"en\" content=\"a\"/><value lang=\"EN\" content=\"b\"/>"),
        ),
        (
            "[en, \"en \"]",
            description("<value lang=\"en\" content=\"a\"/><value lang=\"en \" content=\"b\"/>"),
        ),
        (
            "[en, fr]",
            description("<value lang=\"en\" content=\"a\"/><value lang=\"fr\" content=\"b\"/>"),
        ),
        ("one value", description("<value/>")),
        (
            "no content on any utility kind",
            in_claim(&format!(
                "<description xmi:id=\"d\"/><implementationConstraint xmi:id=\"i\"/>\
                 <note xmi:id=\"n\"/><taggedValue xmi:id=\"t\">{KEY}</taggedValue>"
            )),
        ),
        (
            "a key and a content in the same language",
            in_claim(
                "<taggedValue xmi:id=\"t\"><key><value lang=\"en\" content=\"k\"/></key>\
                 <content><value lang=\"en\" content=\"v\"/></content></taggedValue>",
            ),
        ),
        (
            "two Descriptions in the same language",
            in_claim(
                "<description xmi:id=\"d1\"><content><value lang=\"en\" content=\"a\"/></content></description>\
                 <description xmi:id=\"d2\"><content><value lang=\"en\" content=\"b\"/></content></description>",
            ),
        ),
    ];
    for (what, input) in &cases {
        let found = validation::validate(&read(what, input));
        assert!(found.is_empty(), "{ROW}: {what}: findings {found:?}");
    }
}

// --- Item 4: order and determinism ----------------------------------------------------

#[test]
fn sacm23_val_010_findings_are_in_diagnostic_order_and_deterministic() {
    let input = in_claim(
        "<taggedValue xmi:id=\"t\"><content/></taggedValue>\
         <note xmi:id=\"b\"><content/></note>\
         <description xmi:id=\"a\"><content><value content=\"x\"/><value content=\"y\"/></content></description>",
    );
    let doc = read("several findings", &input);
    let found = findings("several findings", &doc);
    assert_eq!(found.len(), 4, "{ROW}: several findings: {found:?}");
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(
        found, sorted,
        "{ROW}: the findings are not in Diagnostic order"
    );
    let affected: Vec<&str> = found.iter().map(|f| f.affected()[0].as_str()).collect();
    let mut expected = vec!["a", "b", "t", "t"];
    expected.sort_unstable();
    let mut seen = affected.clone();
    seen.sort_unstable();
    assert_eq!(seen, expected, "{ROW}: the affected ids");
    for _ in 0..3 {
        assert_eq!(
            validation::validate(&read("again", &input)),
            found,
            "{ROW}: validate is not deterministic"
        );
    }
}

// --- Item 5: kept, refused strictly, written tolerantly ------------------------------------

#[test]
fn sacm23_val_010_values_are_kept_a_strict_save_refuses_and_a_tolerant_save_writes_as_held() {
    let input = in_claim(
        "<description xmi:id=\"d\"><content><value lang=\"en\" content=\"a\"/>\
         <value lang=\"en\" content=\"b\"/></content></description>\
         <note xmi:id=\"n\"><content/></note>\
         <taggedValue xmi:id=\"t\"><content><value content=\"v\"/></content></taggedValue>",
    );
    let doc = read("invalid MultiLangStrings", &input);
    let before = doc.clone();
    let found = findings("invalid MultiLangStrings", &doc);
    assert_eq!(found.len(), 3, "{ROW}: setup: {found:?}");
    assert_eq!(doc, before, "{ROW}: validate changed the document");
    let held = |doc: &Document, id: &str| doc.get(id).and_then(|e| e.content().cloned());
    assert_eq!(
        held(&doc, "d"),
        Some(
            [
                LangString::new("a").with_lang("en"),
                LangString::new("b").with_lang("en")
            ]
            .into_iter()
            .collect::<MultiLangString>()
        ),
        "{ROW}: the repeated values are not all kept"
    );
    let error = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: a strict save wrote the document:\n{text}"),
        Err(error) => error,
    };
    assert!(
        matches!(&error, WriteError::Invalid { .. }),
        "{ROW}: a strict save is not WriteError::Invalid: {error:?}"
    );
    assert_eq!(
        error.diagnostics(),
        found.as_slice(),
        "{ROW}: the strict refusal does not hold these diagnostics"
    );
    let text = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: a tolerant save refused: {error:?}"),
    };
    let back = read("written tolerantly", &text);
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: a tolerant save does not write the document as held:\n{text}"
    );
    assert_eq!(
        findings("written tolerantly", &back),
        found,
        "{ROW}: the written document validates differently"
    );
}
