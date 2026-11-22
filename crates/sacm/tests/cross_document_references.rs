//! Tests for cross-document references (SACM23-XMI-027; XMI 2.5.1 §7.10.2
//! "The URI reference must be of the form `URI#id_value`, where URI locates the
//! XML file containing the XML element to link to", rule 2l "use the href
//! attribute to specify an XML element in another document", Annex B.2
//! constraint 10; D-PRESERVE; ADR 0040).
//!
//! A reference element of an element reference end whose `href`, trimmed,
//! is `document#fragment` (a non-empty document part, one `#`, a non-empty
//! fragment without `#`, `(` or `)`, no XML white space) is read in both
//! modes and held as a `model::Href` in a list of the end's own
//! (`SacmElement::hrefs`), beside its ids, in document order. It is never
//! resolved. It counts as a value of its end, is never SACM-REF-001, -002 or
//! -003, is written in element form after the end's ids, and is compared by
//! `Difference::Hrefs`. An `href` that names no element keeps its reference
//! element as preserved content with SACM-XMI-003. The other cross-document
//! links are SACM23-XMI-029's.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::CrossPackageReferencePolicy::ApplyReferencePolicy;
use sacm::edit::PackageDeletePolicy::DeleteRecursively;
use sacm::edit::ReferenceDeletePolicy::ScrubReferences;
use sacm::edit::{
    self, AddReference, CopyElements, DeleteElement, EditError, MoveElement, Mutation, Operation,
    RemoveReference, SetReferences,
};
use sacm::model::{Document, ElementId, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

use sacm::compare::Difference;
use sacm::model::Href;

const ROW: &str = "SACM23-XMI-027";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a root: `sacm`, `xmi` and `xlink`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xlink=\"http://www.w3.org/1999/xlink\"",
);

/// The worked fixture and its golden output.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/cross-document-references-valid.sacm.xmi"
);
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/cross-document-references.golden.sacm.xmi"
);

/// The `href` of the samples.
const OTHER: &str = "other.sacm.xmi#x";

// --- Inputs ----------------------------------------------------------------------

/// An element of `class` in the role `role`, `id`, with `attributes`, named
/// after its id, holding `body` after its name.
fn el(role: &str, class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// A root of `class`, `id`, with `attributes`, named after its id, holding
/// `body`.
fn root(class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<sacm:{class}{NS} xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</sacm:{class}>"
    )
}

/// A Claim `id` with `attributes`, holding `body`.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    el("argumentElement", "Claim", id, attributes, body)
}

/// The ArgumentPackage root `p` holding `body`.
fn in_p(body: &str) -> String {
    root("ArgumentPackage", "p", "", body)
}

/// [`in_p`] without the `xlink` declaration, so that a preserved fragment
/// declares no binding of its own (ADR 0033).
fn in_p_without_xlink(body: &str) -> String {
    in_p(body).replacen(" xmlns:xlink=\"http://www.w3.org/1999/xlink\"", "", 1)
}

/// The reference element of `end` naming `href`.
fn link(end: &str, href: &str) -> String {
    format!("<{end} href=\"{href}\"/>")
}

/// The AssuranceCasePackage `case` holding the `ArgumentPackage`s `p` and `q`,
/// each with its interface (`i` implementing `p`, `j` implementing `q`), then
/// `extra`.
fn modular(extra: &str) -> String {
    root(
        "AssuranceCasePackage",
        "case",
        "",
        &format!(
            "{}{}{extra}",
            el(
                "argumentPackage",
                "ArgumentPackage",
                "p",
                "",
                &format!(
                    "{}{}",
                    claim("x", "", ""),
                    el(
                        "argumentElement",
                        "ArgumentPackageInterface",
                        "i",
                        " implements=\"p\"",
                        ""
                    )
                )
            ),
            el(
                "argumentPackage",
                "ArgumentPackage",
                "q",
                "",
                &el(
                    "argumentElement",
                    "ArgumentPackageInterface",
                    "j",
                    " implements=\"q\"",
                    ""
                )
            ),
        ),
    )
}

/// [`modular`] with the `ArgumentPackageBinding` `b`, with `attributes` and
/// `body`.
fn with_b(attributes: &str, body: &str) -> String {
    modular(&el(
        "argumentPackage",
        "ArgumentPackageBinding",
        "b",
        attributes,
        body,
    ))
}

/// The participant elements of `ids`, then of `hrefs`.
fn participants(ids: &[&str], hrefs: &[&str]) -> String {
    ids.iter()
        .map(|id| format!("<participantPackage xmi:idref=\"{id}\"/>"))
        .chain(hrefs.iter().map(|href| link("participantPackage", href)))
        .collect()
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
            continue;
        }
        text.push(c);
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (text, marks)
}

// --- Reading, writing and validating ---------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to `==` documents, and returns the strict one.
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
                panic!("{ROW}: {what}: a {mode:?} read refused it: {error} ({error:?})\n{input}")
            }
        }
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// The strict read's refusal of `input`.
fn refused(what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returned a document of {} elements",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// The tolerant read of `input`, which must succeed.
fn tolerant(what: &str, input: &str) -> ReadOutput {
    match read(Mode::Tolerant, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error} ({error:?})"),
    }
}

fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be `==` and the second write must give the same bytes.
fn round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(what, doc, mode);
    let back = match read(mode, &text) {
        Ok(output) => output.into_document(),
        Err(error) => {
            panic!("{ROW}: {what}: the {mode:?} output does not read back: {error}\n{text}")
        }
    };
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the output does not read back equal\n{text}"
    );
    assert!(back == *doc, "{ROW}: {what}: the output reads back !=");
    assert_eq!(
        write_in(what, &back, mode),
        text,
        "{ROW}: {what}: writing what was read back gives other bytes"
    );
    text
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
}

/// The `href`s of the end `end` of `id`, as strings.
fn hrefs(doc: &Document, id: &str, end: ReferenceEnd) -> Vec<String> {
    element(doc, id)
        .hrefs(end)
        .iter()
        .map(|href| href.as_str().to_owned())
        .collect()
}

/// The (code, severity, row, affected) of each finding of `validate`.
fn findings(doc: &Document) -> Vec<(String, Severity, String, Vec<String>)> {
    validation::validate(doc)
        .iter()
        .map(|one| {
            (
                one.code().as_str().to_owned(),
                one.severity(),
                one.requirement_id().to_owned(),
                one.affected()
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
            )
        })
        .collect()
}

/// The findings of `validate` with the code `code`.
fn of_code(doc: &Document, code: Code) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|one| one.code() == code)
        .collect()
}

/// The text argument `name` of `diagnostic`.
fn argument_of<'d>(diagnostic: &'d Diagnostic, name: &str) -> Option<&'d str> {
    diagnostic
        .arguments()
        .iter()
        .find(|argument| argument.name() == name)
        .and_then(|argument| argument.value().as_text())
}

/// The trimmed lines of the element `id` in `text`: its start tag to its end
/// tag.
fn block<'t>(text: &'t str, id: &str) -> Vec<&'t str> {
    let attribute = format!("xmi:id=\"{id}\"");
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
    };
    let indent = lines[start].len() - lines[start].trim_start().len();
    let mut out = vec![lines[start].trim()];
    for line in &lines[start + 1..] {
        out.push(line.trim());
        if line.len() - line.trim_start().len() == indent {
            break;
        }
    }
    out
}

// --- Edits -----------------------------------------------------------------------

/// Applies `operation`, which the test expects to succeed, after checking
/// that `edit::check` accepts it.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{ROW}: check refuses {operation:?}, which apply accepts"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, with `check` refusing it the same, and returns why.
fn refuse(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        checked,
        Err(error.clone()),
        "{ROW}: check answers {operation:?} otherwise than apply"
    );
    error
}

/// The delete of `element` with `ScrubReferences`, `DeleteRecursively` and
/// `ApplyReferencePolicy`.
fn scrubbing(element: &str) -> DeleteElement {
    DeleteElement::new(element)
        .with_reference_policy(ScrubReferences)
        .with_package_policy(DeleteRecursively)
        .with_cross_package_policy(ApplyReferencePolicy)
}

// =================================================================================
// Reading
// =================================================================================

/// Every element reference end, with a holder `h` that has it: (what, end,
/// the document with `<end href="other.sacm.xmi#x"/>` on `h`).
#[expect(
    clippy::too_many_lines,
    reason = "one input per end and holder reads best as one list"
)]
fn every_end() -> Vec<(String, ReferenceEnd, String)> {
    let on = |end: &str| link(end, OTHER);
    let argument = |class: &str, attributes: &str, end: &str| {
        in_p(&el("argumentElement", class, "h", attributes, &on(end)))
    };
    let artifact = |class: &str, end: &str| {
        root(
            "ArtifactPackage",
            "a",
            "",
            &el("artifactElement", class, "h", "", &on(end)),
        )
    };
    let terminology = |class: &str, attributes: &str, end: &str| {
        root(
            "TerminologyPackage",
            "t",
            "",
            &el("terminologyElement", class, "h", attributes, &on(end)),
        )
    };
    let mut ends = vec![
        (
            "citedElement on a Claim".to_owned(),
            ReferenceEnd::CitedElement,
            argument("Claim", " isCitation=\"true\"", "citedElement"),
        ),
        (
            "abstractForm on a Claim".to_owned(),
            ReferenceEnd::AbstractForm,
            argument("Claim", "", "abstractForm"),
        ),
        (
            "metaClaim on a Claim".to_owned(),
            ReferenceEnd::MetaClaim,
            argument("Claim", "", "metaClaim"),
        ),
        (
            "reasoning on an AssertedInference".to_owned(),
            ReferenceEnd::Reasoning,
            argument("AssertedInference", "", "reasoning"),
        ),
        (
            "source on an AssertedInference".to_owned(),
            ReferenceEnd::Source,
            argument("AssertedInference", "", "source"),
        ),
        (
            "target on an AssertedInference".to_owned(),
            ReferenceEnd::Target,
            argument("AssertedInference", "", "target"),
        ),
        (
            "structure on an ArgumentReasoning".to_owned(),
            ReferenceEnd::Structure,
            argument("ArgumentReasoning", "", "structure"),
        ),
        (
            "referencedArtifactElement on an ArtifactReference".to_owned(),
            ReferenceEnd::ReferencedArtifactElement,
            argument("ArtifactReference", "", "referencedArtifactElement"),
        ),
        (
            "argumentElement on an ArgumentGroup".to_owned(),
            ReferenceEnd::GroupMember,
            argument("ArgumentGroup", "", "argumentElement"),
        ),
        (
            "artifactElement on an ArtifactGroup".to_owned(),
            ReferenceEnd::ArtifactGroupMember,
            artifact("ArtifactGroup", "artifactElement"),
        ),
        (
            "terminologyElement on a TerminologyGroup".to_owned(),
            ReferenceEnd::TerminologyGroupMember,
            terminology("TerminologyGroup", "", "terminologyElement"),
        ),
        (
            "source on an ArtifactAssetRelationship".to_owned(),
            ReferenceEnd::ArtifactSource,
            artifact("ArtifactAssetRelationship", "source"),
        ),
        (
            "target on an ArtifactAssetRelationship".to_owned(),
            ReferenceEnd::ArtifactTarget,
            artifact("ArtifactAssetRelationship", "target"),
        ),
        (
            "category on a Term".to_owned(),
            ReferenceEnd::Category,
            terminology("Term", " value=\"h\"", "category"),
        ),
        (
            "origin on a Term".to_owned(),
            ReferenceEnd::Origin,
            terminology("Term", " value=\"h\"", "origin"),
        ),
        (
            "element on an Expression".to_owned(),
            ReferenceEnd::Element,
            terminology("Expression", " value=\"h\"", "element"),
        ),
        (
            "interface on an ArgumentPackage".to_owned(),
            ReferenceEnd::Interface,
            root("ArgumentPackage", "h", "", &on("interface")),
        ),
    ];
    for family in ["AssuranceCase", "Terminology", "Argument", "Artifact"] {
        ends.push((
            format!("implements on an {family}PackageInterface"),
            ReferenceEnd::Implements,
            root(
                &format!("{family}PackageInterface"),
                "h",
                "",
                &on("implements"),
            ),
        ));
        ends.push((
            format!("participantPackage on an {family}PackageBinding"),
            ReferenceEnd::ParticipantPackage,
            root(
                &format!("{family}PackageBinding"),
                "h",
                "",
                &on("participantPackage"),
            ),
        ));
    }
    ends
}

#[test]
fn sacm23_xmi_027_an_href_with_a_document_part_reads_on_every_end_in_both_modes() {
    let ends = every_end();
    let covered: Vec<ReferenceEnd> = ReferenceEnd::ALL
        .iter()
        .copied()
        .filter(|end| ends.iter().any(|(_, found, _)| found == end))
        .collect();
    assert_eq!(
        covered.len() + 1,
        ReferenceEnd::ALL.len(),
        "{ROW}: setup: the inputs do not cover every end but the derived expression"
    );
    for (what, end, input) in &ends {
        let doc = read_ok(what, input);
        assert!(
            element(&doc, "h").references(*end).is_empty(),
            "{ROW}: {what}: the href is held as an id: {:?}",
            element(&doc, "h").references(*end)
        );
        assert!(
            !doc.elements().any(SacmElement::has_preserved_content),
            "{ROW}: {what}: the href is kept as preserved content"
        );
        {
            assert_eq!(
                hrefs(&doc, "h", *end),
                [OTHER],
                "{ROW}: {what}: hrefs({end:?})"
            );
            for &other in ReferenceEnd::ALL.iter().filter(|other| **other != *end) {
                assert!(
                    element(&doc, "h").hrefs(other).is_empty(),
                    "{ROW}: {what}: the end {other:?} holds an href"
                );
            }
        }
    }
}

#[test]
fn sacm23_xmi_027_the_value_is_held_trimmed_and_split_at_its_hash() {
    let on_claim = |value: &str| {
        in_p(&claim(
            "c",
            " isCitation=\"true\"",
            &format!("<citedElement href=\"{value}\"/>"),
        ))
    };
    let cases = [
        (
            " other.sacm.xmi#x ",
            "other.sacm.xmi#x",
            "other.sacm.xmi",
            "x",
        ),
        (
            "&#9;other.sacm.xmi#x&#10;",
            "other.sacm.xmi#x",
            "other.sacm.xmi",
            "x",
        ),
        (
            "\tother.sacm.xmi#x\n",
            "other.sacm.xmi#x",
            "other.sacm.xmi",
            "x",
        ),
        (
            "http://example.org/a/b.xmi#c_1",
            "http://example.org/a/b.xmi#c_1",
            "http://example.org/a/b.xmi",
            "c_1",
        ),
        (
            "b.xmi#//@argumentElement.0",
            "b.xmi#//@argumentElement.0",
            "b.xmi",
            "//@argumentElement.0",
        ),
        ("a%20b.xmi#c", "a%20b.xmi#c", "a%20b.xmi", "c"),
    ];
    for (value, held, document, fragment) in cases {
        let what = format!("href={value:?}");
        let doc = read_ok(&what, &on_claim(value));
        assert_eq!(
            element(&doc, "c").cited_element(),
            None,
            "{ROW}: {what}: held as an id"
        );
        {
            let found = element(&doc, "c").hrefs(ReferenceEnd::CitedElement);
            let [href] = found else {
                panic!("{ROW}: {what}: not one href: {found:?}");
            };
            assert_eq!(
                (href.as_str(), href.document(), href.fragment()),
                (held, document, fragment),
                "{ROW}: {what}: as_str, document and fragment"
            );
            assert_eq!(
                (href.to_string(), AsRef::<str>::as_ref(href)),
                (held.to_owned(), held),
                "{ROW}: {what}: Display and AsRef are as_str"
            );
            assert_eq!(
                format!("{href:>60}"),
                format!("{held:>60}"),
                "{ROW}: {what}: Display pads as a str does"
            );
        }
    }
    // Ord and Eq are the string's.
    let doc = read_ok(
        "three participant hrefs",
        &with_b("", &participants(&[], &["b.xmi#q", "a.xmi#p", "b.xmi#q"])),
    );
    {
        fn value<T: Clone + std::fmt::Debug + Eq + Ord + std::hash::Hash + Send + Sync>() {}
        value::<Href>();
        let found = element(&doc, "b").hrefs(ReferenceEnd::ParticipantPackage);
        assert_eq!(found.len(), 3, "{ROW}: setup: the three hrefs");
        assert_eq!(
            (
                found[1].cmp(&found[0]),
                found[0].cmp(&found[2]),
                found[0] == found[2]
            ),
            ("a.xmi#p".cmp("b.xmi#q"), std::cmp::Ordering::Equal, true),
            "{ROW}: Ord and Eq are not the string's"
        );
        let mut sorted = found.to_vec();
        sorted.sort();
        assert_eq!(
            sorted.iter().map(Href::as_str).collect::<Vec<_>>(),
            ["a.xmi#p", "b.xmi#q", "b.xmi#q"],
            "{ROW}: sorting by Ord"
        );
    }
}

#[test]
fn sacm23_xmi_027_ids_and_hrefs_of_one_end_are_two_lists_in_document_order() {
    let body = format!(
        "{}<participantPackage xmi:idref=\"i\"/>{}{}",
        link("participantPackage", "b.xmi#q"),
        link("participantPackage", "a.xmi#p"),
        link("participantPackage", "b.xmi#q")
    );
    let doc = read_ok("href, idref, href, href", &with_b("", &body));
    assert_eq!(
        strs(element(&doc, "b").participant_packages()),
        ["i"],
        "{ROW}: participant_packages()"
    );
    assert_eq!(
        hrefs(&doc, "b", ReferenceEnd::ParticipantPackage),
        ["b.xmi#q", "a.xmi#p", "b.xmi#q"],
        "{ROW}: hrefs(ParticipantPackage) in document order, repeats included"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_027_an_href_is_an_occurrence_of_its_end() {
    // implements: an idref and an href in either order, or two hrefs.
    let interface = |body: &str| {
        in_p(&el(
            "argumentElement",
            "ArgumentPackageInterface",
            "i",
            "",
            body,
        ))
    };
    for (what, body, first_id, first_href) in [
        (
            "an idref then an href",
            format!("{MARK}<implements xmi:idref=\"p\"/>{MARK}<implements href=\"o.xmi#p\"/>"),
            Some("p"),
            None,
        ),
        (
            "an href then an idref",
            format!("{MARK}<implements href=\"o.xmi#p\"/>{MARK}<implements xmi:idref=\"p\"/>"),
            None,
            Some("o.xmi#p"),
        ),
        (
            "two hrefs",
            format!("{MARK}<implements href=\"o.xmi#p\"/>{MARK}<implements href=\"o.xmi#q\"/>"),
            None,
            Some("o.xmi#p"),
        ),
    ] {
        let (input, marks) = marked(&interface(&body));
        let error = refused(what, &input);
        assert!(
            matches!(
                &error,
                ReadError::RepeatedValue { element, property, .. }
                    if element.as_str() == "i" && property == "implements"
            ),
            "{ROW}: {what}: not RepeatedValue {{ i, implements }}: {error:?}"
        );
        assert_eq!(
            (error.code(), error.template(), error.locations()),
            (
                Code::MULTIPLICITY_VIOLATION,
                "SACM-MULT-001/repeated-value",
                marks.as_slice()
            ),
            "{ROW}: {what}: not SACM-MULT-001/repeated-value at both occurrences"
        );
        assert_eq!(
            error
                .arguments()
                .iter()
                .find(|argument| argument.name() == "count")
                .and_then(|argument| argument.value().as_count()),
            Some(2),
            "{ROW}: {what}: the count"
        );
        let output = tolerant(what, &input);
        assert_eq!(
            output
                .diagnostics()
                .iter()
                .map(|one| (
                    one.code(),
                    one.severity(),
                    one.template(),
                    one.location(),
                    strs(one.affected())
                ))
                .collect::<Vec<_>>(),
            [(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Warning,
                "SACM-MULT-001/repeat-kept",
                marks.get(1).copied(),
                vec!["i"]
            )],
            "{ROW}: {what}: a tolerant read does not keep the second with repeat-kept"
        );
        let doc = output.into_document();
        assert!(
            element(&doc, "i").has_preserved_content(),
            "{ROW}: {what}: the second occurrence is not kept"
        );
        assert_eq!(
            element(&doc, "i").implements().map(ElementId::as_str),
            first_id,
            "{ROW}: {what}: the id a tolerant read holds"
        );
        assert_eq!(
            hrefs(&doc, "i", ReferenceEnd::Implements),
            first_href.into_iter().collect::<Vec<_>>(),
            "{ROW}: {what}: the href a tolerant read holds"
        );
    }
    // participantPackage as an attribute and an href element.
    let what = "participantPackage=\"i\" and an href element";
    let (input, marks) = marked(&with_b(
        &format!(" {MARK}participantPackage=\"i\""),
        &format!("{MARK}<participantPackage href=\"o.xmi#p\"/>"),
    ));
    let error = refused(what, &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == "b" && property == "participantPackage"
        ),
        "{ROW}: {what}: not RepeatedValue {{ b, participantPackage }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.template(), error.locations()),
        (
            Code::MULTIPLICITY_VIOLATION,
            "SACM-MULT-001/both-forms",
            marks.as_slice()
        ),
        "{ROW}: {what}: not SACM-MULT-001/both-forms at both forms"
    );
    let output = tolerant(what, &input);
    assert_eq!(
        output
            .diagnostics()
            .iter()
            .map(|one| (
                one.code(),
                one.severity(),
                one.requirement_id().to_owned(),
                one.location()
            ))
            .collect::<Vec<_>>(),
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "LIB-COMPAT-001".to_owned(),
            marks.get(1).copied()
        )],
        "{ROW}: {what}: a tolerant read does not give one SACM-MULT-001 Warning at the element"
    );
    let doc = output.into_document();
    assert_eq!(
        strs(element(&doc, "b").participant_packages()),
        ["i"],
        "{ROW}: {what}: a tolerant read does not keep the attribute"
    );
    assert!(
        element(&doc, "b").has_preserved_content(),
        "{ROW}: {what}: the href element is not kept"
    );
    assert!(
        element(&doc, "b")
            .hrefs(ReferenceEnd::ParticipantPackage)
            .is_empty(),
        "{ROW}: {what}: the kept href element is also read"
    );
}

#[test]
fn sacm23_xmi_027_an_href_that_names_no_element_is_preserved_with_xmi_003() {
    for value in [
        "",
        "#",
        "other.xmi",
        "other.xmi#",
        "#a b",
        "other.xmi#a b",
        "other.xmi#a#b",
        "a b.xmi#c",
        "a&#9;b.xmi#c",
    ] {
        let what = format!("href={value:?} on citedElement");
        let (input, marks) = marked(&in_p_without_xlink(&claim(
            "c1",
            " isCitation=\"true\"",
            &format!("{MARK}<citedElement href=\"{value}\"/>"),
        )));
        let mut documents = Vec::new();
        for (mode, severity) in [
            (Mode::Strict, Severity::Error),
            (Mode::Tolerant, Severity::Warning),
        ] {
            let output = match read(mode, &input) {
                Ok(output) => output,
                Err(error) => panic!(
                    "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: \
                     {error} ({error:?})"
                ),
            };
            assert_eq!(
                output
                    .diagnostics()
                    .iter()
                    .map(|one| (
                        one.code(),
                        one.severity(),
                        one.location(),
                        strs(one.affected())
                    ))
                    .collect::<Vec<_>>(),
                [(
                    Code::XMI_UNKNOWN_CONTENT,
                    severity,
                    marks.first().copied(),
                    vec!["c1"]
                )],
                "{ROW}: {what}: not one SACM-XMI-003 at the reference element in {mode:?} mode"
            );
            assert!(
                output
                    .diagnostics()
                    .iter()
                    .all(|one| one.code() != Code::XMI_EXTERNAL_HREF),
                "{ROW}: {what}: SACM-XMI-007 in {mode:?} mode"
            );
            documents.push(output.into_document());
        }
        assert!(
            documents[0] == documents[1],
            "{ROW}: {what}: the modes read different documents"
        );
        let doc = documents.swap_remove(0);
        let c1 = element(&doc, "c1");
        assert!(
            c1.has_preserved_content() && c1.cited_element().is_none(),
            "{ROW}: {what}: not kept as preserved content of c1 with the end empty"
        );
        assert!(
            c1.hrefs(ReferenceEnd::CitedElement).is_empty(),
            "{ROW}: {what}: the end holds an href"
        );
        match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
            Ok(text) => panic!("{ROW}: {what}: a strict write accepted it:\n{text}"),
            Err(error) => assert!(
                matches!(error, WriteError::PreservedContent { .. })
                    && error.code() == Code::XMI_STRICT_SAVE_REFUSED,
                "{ROW}: {what}: a strict write is not PreservedContent (SACM-XMI-006): {error:?}"
            ),
        }
        let text = write_in(&what, &doc, Mode::Tolerant);
        assert!(
            text.contains(&format!("<citedElement href=\"{value}\"")),
            "{ROW}: {what}: a tolerant write does not write it back\n{text}"
        );
    }
}

#[test]
fn sacm23_xmi_027_an_attribute_token_with_a_hash_is_an_id() {
    let what = "participantPackage=\"other.xmi#p i\"";
    let doc = read_ok(what, &with_b(" participantPackage=\"other.xmi#p i\"", ""));
    assert_eq!(
        strs(element(&doc, "b").participant_packages()),
        ["other.xmi#p", "i"],
        "{ROW}: {what}: the ids"
    );
    assert!(
        element(&doc, "b")
            .hrefs(ReferenceEnd::ParticipantPackage)
            .is_empty(),
        "{ROW}: {what}: a token is held as an href"
    );
    assert_eq!(
        findings(&doc),
        [(
            "SACM-REF-001".to_owned(),
            Severity::Error,
            "SACM23-VAL-003".to_owned(),
            vec!["b".to_owned(), "other.xmi#p".to_owned()]
        )],
        "{ROW}: {what}: not one SACM-REF-001"
    );
    match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => panic!("{ROW}: {what}: a strict write accepted it:\n{text}"),
        Err(error) => assert!(
            matches!(error, WriteError::Invalid { .. }),
            "{ROW}: {what}: a strict write is not Invalid (schema): {error:?}"
        ),
    }
    let text = write_in(what, &doc, Mode::Tolerant);
    assert!(
        text.lines().any(|line| line.contains("xmi:id=\"b\"")
            && line.contains(" participantPackage=\"other.xmi#p i\"")),
        "{ROW}: {what}: a tolerant write does not write the attribute back\n{text}"
    );
}

// =================================================================================
// Writing
// =================================================================================

#[test]
fn sacm23_xmi_027_an_end_with_an_href_is_written_in_element_form_by_both_saves() {
    let body = format!(
        "{}{}{}{}",
        claim("c1", "", ""),
        claim("c3", " metaClaim=\"c1\"", ""),
        el("argumentElement", "ArgumentReasoning", "r", "", ""),
        el(
            "argumentElement",
            "AssertedInference",
            "x",
            " isCitation=\"true\" reasoning=\"r\"",
            "<target href=\"o.xmi#t\"/><source href=\"o.xmi#s\"/><source xmi:idref=\"c1\"/>\
             <citedElement href=\"a.xmi?x=1&amp;y=2#c\"/>"
        )
    );
    let doc = read_ok("an AssertedInference with hrefs", &in_p(&body));
    let strict = round_trip("an AssertedInference with hrefs", &doc, Mode::Strict);
    let tolerant = round_trip("an AssertedInference with hrefs", &doc, Mode::Tolerant);
    assert_eq!(
        strict, tolerant,
        "{ROW}: the strict and tolerant writes differ"
    );
    assert_eq!(
        block(&strict, "x"),
        [
            "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"x\" isCitation=\"true\" reasoning=\"r\">",
            "<name content=\"x\" />",
            "<citedElement href=\"a.xmi?x=1&amp;y=2#c\" />",
            "<source xmi:idref=\"c1\" />",
            "<source href=\"o.xmi#s\" />",
            "<target href=\"o.xmi#t\" />",
            "</argumentElement>",
        ],
        "{ROW}: not written as name, then each end in ReferenceEnd::ALL order, ids then hrefs\n{strict}"
    );
    assert_eq!(
        block(&strict, "c3"),
        [
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\" metaClaim=\"c1\">",
            "<name content=\"c3\" />",
            "</argumentElement>",
        ],
        "{ROW}: an end with ids only is not written as an attribute\n{strict}"
    );
    {
        let back = read_ok("the output", &strict);
        assert_eq!(
            hrefs(&back, "x", ReferenceEnd::CitedElement),
            ["a.xmi?x=1&y=2#c"],
            "{ROW}: the escaped href does not read back equal"
        );
        assert_eq!(
            element(&back, "x")
                .hrefs(ReferenceEnd::CitedElement)
                .first()
                .map(Href::document),
            Some("a.xmi?x=1&y=2"),
            "{ROW}: the document part of the escaped href"
        );
    }
}

#[test]
fn sacm23_xmi_027_the_fixture_reads_validates_and_writes_exactly_the_golden_file() {
    let fixture = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"));
    let doc = read_ok("the fixture", &fixture);
    assert_eq!(
        findings(&doc),
        [],
        "{ROW}: the fixture: validate reports a finding"
    );
    for mode in MODES {
        assert_eq!(
            write_in("the fixture", &doc, mode),
            golden,
            "{ROW}: a {mode:?} write of the fixture is not exactly the golden file"
        );
    }
    let back = read_ok("the golden file", &golden);
    assert!(
        back == doc,
        "{ROW}: the golden file does not read == to the fixture: {:?}",
        semantic_compare(&doc, &back)
    );
    assert_eq!(
        write_in("the golden file", &back, Mode::Strict),
        golden,
        "{ROW}: writing the golden file again gives other bytes"
    );
    assert_eq!(
        (
            strs(element(&doc, "binding").participant_packages()),
            element(&doc, "binding_vehicle")
                .cited_element()
                .map(ElementId::as_str),
            element(&doc, "binding_braking").cited_element(),
        ),
        (vec!["vehicle_api"], Some("vehicle_safe_cited"), None),
        "{ROW}: the fixture's ids"
    );
    assert_eq!(
        (
            hrefs(&doc, "binding", ReferenceEnd::ParticipantPackage),
            hrefs(&doc, "binding_braking", ReferenceEnd::CitedElement),
        ),
        (
            vec!["braking.sacm.xmi#braking_api".to_owned()],
            vec!["braking.sacm.xmi#braking_safe_cited".to_owned()],
        ),
        "{ROW}: the fixture's hrefs"
    );
}

// =================================================================================
// Validating
// =================================================================================

#[test]
fn sacm23_xmi_027_an_href_is_never_dangling_typed_or_a_referrer() {
    // Every kind of end, given an href whose fragment is the id `x` of a
    // Claim of this document: no SACM-REF-001, -002 or -003.
    let input = with_b(
        "",
        &format!(
            "{}{}",
            participants(&["i"], &["o.xmi#x"]),
            claim(
                "h",
                " isCitation=\"true\"",
                &link("citedElement", "o.xmi#x")
            )
        ),
    );
    let mut doc = read_ok("hrefs naming x", &input);
    for (what, end, input) in every_end() {
        let other = read_ok(&what, &input);
        let codes: Vec<String> = findings(&other)
            .into_iter()
            .map(|(code, ..)| code)
            .filter(|code| code.starts_with("SACM-REF-"))
            .collect();
        assert_eq!(
            codes,
            Vec::<String>::new(),
            "{ROW}: {what} ({end:?}): a SACM-REF finding"
        );
    }
    assert_eq!(
        findings(&doc),
        [],
        "{ROW}: hrefs naming x: validate reports a finding"
    );
    assert!(
        doc.referrers("x").is_empty(),
        "{ROW}: an href is a referrer of the local x"
    );
    let delete = DeleteElement::new("x");
    let preview = match edit::preview(&doc, &delete.clone().into()) {
        Ok(preview) => preview,
        Err(error) => panic!("{ROW}: the preview of deleting x was refused: {error}"),
    };
    assert!(
        preview.dangling_references().is_empty(),
        "{ROW}: deleting x lists a dangling reference: {:?}",
        preview.dangling_references()
    );
    let mutation = applied(&mut doc, delete);
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["x"], Vec::<&str>::new()),
        "{ROW}: deleting x changes another element"
    );
    assert_eq!(
        (
            hrefs(&doc, "b", ReferenceEnd::ParticipantPackage),
            hrefs(&doc, "h", ReferenceEnd::CitedElement)
        ),
        (vec!["o.xmi#x".to_owned()], vec!["o.xmi#x".to_owned()]),
        "{ROW}: the hrefs after deleting x"
    );
}

#[test]
fn sacm23_xmi_027_rules_that_need_a_value_count_an_href() {
    // source and target only as hrefs: no SACM-MULT-001.
    for (what, input) in [
        (
            "an AssertedInference",
            in_p(&el(
                "argumentElement",
                "AssertedInference",
                "r",
                "",
                &format!("{}{}", link("source", "o.xmi#s"), link("target", "o.xmi#t")),
            )),
        ),
        (
            "an ArtifactAssetRelationship",
            root(
                "ArtifactPackage",
                "a",
                "",
                &el(
                    "artifactElement",
                    "ArtifactAssetRelationship",
                    "r",
                    "",
                    &format!("{}{}", link("source", "o.xmi#s"), link("target", "o.xmi#t")),
                ),
            ),
        ),
    ] {
        let what = format!("{what} whose source and target are hrefs");
        let doc = read_ok(&what, &input);
        assert_eq!(
            findings(&doc),
            [],
            "{ROW}: {what}: validate reports a finding"
        );
    }
    // A citedElement href without isCitation: SACM-CITE-001, target the href.
    let what = "a Claim with an href citedElement and no isCitation";
    let doc = read_ok(
        what,
        &in_p(&claim("c", "", &link("citedElement", "o.xmi#c"))),
    );
    let found = of_code(&doc, Code::CITATION_INVALID);
    assert_eq!(
        (
            found.len(),
            found.first().map(|one| strs(one.affected())),
            found.first().and_then(|one| argument_of(one, "target"))
        ),
        (1, Some(vec!["c"]), Some("o.xmi#c")),
        "{ROW}: {what}: not one SACM-CITE-001 with the target o.xmi#c"
    );
    assert_eq!(findings(&doc).len(), 1, "{ROW}: {what}: another finding");
    // An asCited citation by href: no SACM-CITE-002.
    let what = "an asCited citation by href";
    let doc = read_ok(
        what,
        &in_p(&claim(
            "c",
            " isCitation=\"true\" assertionDeclaration=\"asCited\"",
            &link("citedElement", "o.xmi#c"),
        )),
    );
    assert_eq!(
        findings(&doc),
        [],
        "{ROW}: {what}: validate reports a finding"
    );
    // An abstract Claim with an href abstractForm: one abstract-holder Error.
    let what = "an abstract Claim with an href abstractForm";
    let doc = read_ok(
        what,
        &in_p(&claim(
            "c",
            " isAbstract=\"true\"",
            &link("abstractForm", "o.xmi#a"),
        )),
    );
    let found = validation::validate(&doc);
    assert_eq!(
        found
            .iter()
            .map(|one| (one.template(), one.severity(), argument_of(one, "target")))
            .collect::<Vec<_>>(),
        [(
            "SACM-ABS-001/abstract-holder",
            Severity::Error,
            Some("o.xmi#a")
        )],
        "{ROW}: {what}: not one SACM-ABS-001 abstract-holder Error"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "one input per rule reads best as one list"
)]
#[test]
fn sacm23_xmi_027_rules_that_need_the_target_skip_an_href() {
    let interface = |attributes: &str, body: &str| {
        el(
            "argumentElement",
            "ArgumentPackageInterface",
            "i",
            attributes,
            body,
        )
    };
    for (what, row, input) in [
        (
            "a concrete Claim with an href abstractForm",
            "SACM23-VAL-008",
            in_p(&claim("c", "", &link("abstractForm", "o.xmi#a"))),
        ),
        (
            "an asCited citation by href",
            "SACM23-VAL-019",
            in_p(&claim(
                "c",
                " isCitation=\"true\" assertionDeclaration=\"asCited\"",
                &link("citedElement", "o.xmi#r"),
            )),
        ),
        (
            "an AssertedEvidence whose source is an href",
            "SACM23-VAL-016",
            in_p(&format!(
                "{}{}",
                claim("c", "", ""),
                el(
                    "argumentElement",
                    "AssertedEvidence",
                    "e",
                    " target=\"c\"",
                    &link("source", "o.xmi#s")
                )
            )),
        ),
        (
            "an AssertedArtifactSupport whose source and target are hrefs",
            "SACM23-VAL-016",
            in_p(&el(
                "argumentElement",
                "AssertedArtifactSupport",
                "e",
                "",
                &format!("{}{}", link("source", "o.xmi#s"), link("target", "o.xmi#t")),
            )),
        ),
        (
            "a concrete Expression whose element is an href",
            "SACM23-VAL-017",
            root(
                "TerminologyPackage",
                "t",
                "",
                &el(
                    "terminologyElement",
                    "Expression",
                    "e",
                    " value=\"e\"",
                    &link("element", "o.xmi#t"),
                ),
            ),
        ),
        (
            "an interface citation by href",
            "SACM23-VAL-013",
            in_p(&interface(
                " implements=\"p\"",
                &claim(
                    "ci",
                    " isCitation=\"true\"",
                    &link("citedElement", "o.xmi#c"),
                ),
            )),
        ),
        (
            "an interface whose implements is an href",
            "SACM23-VAL-023",
            root(
                "AssuranceCasePackage",
                "case",
                "",
                &el(
                    "argumentPackage",
                    "ArgumentPackageInterface",
                    "i",
                    "",
                    &link("implements", "o.xmi#p"),
                ),
            ),
        ),
    ] {
        let doc = read_ok(what, &input);
        assert_eq!(
            findings(&doc),
            [],
            "{ROW}: {what}: validate reports a finding ({row} checks the target)"
        );
    }
}

#[test]
fn sacm23_xmi_027_a_repeated_href_is_one_val_022_warning() {
    let what = "ids [i] and hrefs [o.xmi#p, o.xmi#p]";
    let doc = read_ok(
        what,
        &with_b("", &participants(&["i"], &["o.xmi#p", "o.xmi#p"])),
    );
    let found = validation::validate(&doc);
    assert_eq!(
        found
            .iter()
            .map(|one| (
                one.code(),
                one.severity(),
                one.requirement_id(),
                one.clause(),
                strs(one.affected()),
                one.template()
            ))
            .collect::<Vec<_>>(),
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022",
            Some("11.5"),
            vec!["b"],
            "SACM-MULT-001/repeated-reference"
        )],
        "{ROW}: {what}: not one SACM23-VAL-022 Warning affecting b, and no SACM23-VAL-014 finding"
    );
    assert_eq!(
        found[0].message(),
        "ArgumentPackageBinding \"b\" holds \"o.xmi#p\" 2 times in its participantPackage, whose \
         ids are unique in SACM 2.3 (the repeat is kept)",
        "{ROW}: {what}: the message"
    );
    assert_eq!(
        argument_of(&found[0], "target"),
        Some("o.xmi#p"),
        "{ROW}: {what}: the target is not the href"
    );
    // Three hrefs, one of them repeated: one Warning, for the repeat only.
    let what = "ids [i] and hrefs [o.xmi#p, o.xmi#q, o.xmi#p]";
    let doc = read_ok(
        what,
        &with_b(
            "",
            &participants(&["i"], &["o.xmi#p", "o.xmi#q", "o.xmi#p"]),
        ),
    );
    let found = of_code(&doc, Code::MULTIPLICITY_VIOLATION);
    assert_eq!(
        found
            .iter()
            .map(|one| (
                one.severity(),
                one.requirement_id(),
                strs(one.affected()),
                argument_of(one, "target"),
                one.arguments()
                    .iter()
                    .find(|argument| argument.name() == "count")
                    .and_then(|argument| argument.value().as_count())
            ))
            .collect::<Vec<_>>(),
        [(
            Severity::Warning,
            "SACM23-VAL-022",
            vec!["b"],
            Some("o.xmi#p"),
            Some(2)
        )],
        "{ROW}: {what}: not one SACM23-VAL-022 Warning affecting b, for o.xmi#p held 2 times"
    );
}

// =================================================================================
// Comparing
// =================================================================================

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_xmi_027_compare_reports_hrefs() {
    // Left: ids [i, j] and the href o.xmi#p; right: ids [i, j] only.
    let left = read_ok(
        "left",
        &with_b("", &participants(&["i", "j"], &["o.xmi#p"])),
    );
    let right = read_ok("right", &with_b(" participantPackage=\"i j\"", ""));
    let found = semantic_compare(&left, &right);
    let [
        Difference::Hrefs {
            id,
            end,
            left: l,
            right: r,
        },
    ] = found.as_slice()
    else {
        panic!("{ROW}: not one Hrefs difference: {found:?}");
    };
    assert_eq!(
        (
            id.as_str(),
            *end,
            l.iter().map(Href::as_str).collect::<Vec<_>>(),
            r.iter().map(Href::as_str).collect::<Vec<_>>()
        ),
        (
            "b",
            ReferenceEnd::ParticipantPackage,
            vec!["o.xmi#p"],
            Vec::<&str>::new()
        ),
        "{ROW}: the Hrefs difference"
    );
    assert_eq!(
        found[0].to_string(),
        "element \"b\" differs in the hrefs of participantPackage: [\"o.xmi#p\"] on the left, \
         [] on the right",
        "{ROW}: the Display"
    );
    assert_eq!(
        found[0].element_id().map(ElementId::as_str),
        Some("b"),
        "{ROW}: element_id()"
    );
    assert!(
        left != right,
        "{ROW}: documents that differ in hrefs are =="
    );
    // Swapping the documents swaps the sides.
    let swapped = semantic_compare(&right, &left);
    assert!(
        matches!(
            swapped.as_slice(),
            [Difference::Hrefs { left, right, .. }] if left.is_empty() && right.len() == 1
        ),
        "{ROW}: the swapped comparison: {swapped:?}"
    );
    // Swapped href order: one difference, in order.
    let ab = read_ok(
        "ab",
        &with_b("", &participants(&["i"], &["a.xmi#p", "b.xmi#q"])),
    );
    let ba = read_ok(
        "ba",
        &with_b("", &participants(&["i"], &["b.xmi#q", "a.xmi#p"])),
    );
    let found = semantic_compare(&ab, &ba);
    assert!(
        matches!(found.as_slice(), [Difference::Hrefs { .. }]) && ab != ba,
        "{ROW}: hrefs in another order are not one Hrefs difference: {found:?}"
    );
    assert_eq!(
        found[0].to_string(),
        "element \"b\" differs in the hrefs of participantPackage: [\"a.xmi#p\", \"b.xmi#q\"] on \
         the left, [\"b.xmi#q\", \"a.xmi#p\"] on the right",
        "{ROW}: the Display of hrefs in another order"
    );
    // `==` holds exactly when there is no difference.
    let again = read_ok(
        "ab again",
        &with_b("", &participants(&["i"], &["a.xmi#p", "b.xmi#q"])),
    );
    assert!(
        semantic_compare(&ab, &again).is_empty() && ab == again,
        "{ROW}: equal hrefs are not =="
    );
    // The order: after ParticipantPackages, one per end in ReferenceEnd::ALL
    // order, before Name.
    let left = read_ok(
        "left of four differences",
        &with_b(
            " isCitation=\"true\"",
            &format!(
                "{}{}",
                link("citedElement", "o.xmi#c"),
                participants(&["i"], &["o.xmi#p"])
            ),
        ),
    );
    let right = read_ok(
        "right of four differences",
        &with_b(
            " isCitation=\"true\"",
            &format!(
                "{}{}",
                link("citedElement", "o.xmi#d"),
                participants(&["i", "j"], &["o.xmi#q"])
            ),
        )
        .replacen("<name content=\"b\"/>", "<name content=\"B\"/>", 1),
    );
    let found = semantic_compare(&left, &right);
    let order: Vec<String> = found
        .iter()
        .map(|difference| match difference {
            Difference::ParticipantPackages { .. } => "ParticipantPackages".to_owned(),
            Difference::Hrefs { end, .. } => format!("Hrefs({})", end.name()),
            Difference::Name { .. } => "Name".to_owned(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        order,
        [
            "ParticipantPackages",
            "Hrefs(citedElement)",
            "Hrefs(participantPackage)",
            "Name"
        ],
        "{ROW}: the order of the differences"
    );
}

// =================================================================================
// Editing
// =================================================================================

/// [`with_b`] of ids `[i]` and the href `o.xmi#p`, with the interface `e` in
/// `p` whose `implements` is the href `o.xmi#p`.
fn edit_case() -> Document {
    read_ok(
        "the edit case",
        &with_b("", &participants(&["i"], &["o.xmi#p"])).replacen(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">",
            &format!(
                "{}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">",
                el(
                    "argumentElement",
                    "ArgumentPackageInterface",
                    "e",
                    "",
                    &link("implements", "o.xmi#p")
                )
            ),
            1,
        ),
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_xmi_027_reference_edits_count_hrefs_and_set_references_replaces_them() {
    let mut doc = edit_case();
    assert!(
        element(&doc, "e").implements().is_none(),
        "{ROW}: setup: e holds an implements id"
    );
    applied(
        &mut doc,
        AddReference::new("b", ReferenceEnd::ParticipantPackage, "j"),
    );
    assert_eq!(
        strs(element(&doc, "b").participant_packages()),
        ["i", "j"],
        "{ROW}: AddReference of j"
    );
    assert_eq!(
        hrefs(&doc, "b", ReferenceEnd::ParticipantPackage),
        ["o.xmi#p"],
        "{ROW}: AddReference does not keep the href"
    );
    let mut doc = edit_case();
    let error = refuse(
        &mut doc,
        RemoveReference::new("b", ReferenceEnd::ParticipantPackage, "i"),
    );
    assert!(
        matches!(
            error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{ROW}: RemoveReference of i, leaving the href: {error:?}"
    );
    // A repeated href is one value (ADR 0040 D5).
    let mut repeated = read_ok(
        "ids [i] and hrefs [o.xmi#p, o.xmi#p]",
        &with_b("", &participants(&["i"], &["o.xmi#p", "o.xmi#p"])),
    );
    let error = refuse(
        &mut repeated,
        RemoveReference::new("b", ReferenceEnd::ParticipantPackage, "i"),
    );
    assert!(
        matches!(
            error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{ROW}: RemoveReference of i, leaving one href held twice: {error:?}"
    );
    let error = refuse(
        &mut doc,
        AddReference::new("e", ReferenceEnd::Implements, "p"),
    );
    assert!(
        matches!(
            error,
            EditError::TooManyReferences {
                end: "implements",
                given: 2,
                allowed: 1,
                ..
            }
        ),
        "{ROW}: AddReference to an implements that holds an href: {error:?}"
    );
    let mutation = applied(
        &mut doc,
        SetReferences::new("e", ReferenceEnd::Implements, ["p"]),
    );
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(&doc, "e").implements().map(ElementId::as_str)
        ),
        (vec!["e"], Some("p")),
        "{ROW}: SetReferences of implements"
    );
    assert!(
        element(&doc, "e")
            .hrefs(ReferenceEnd::Implements)
            .is_empty(),
        "{ROW}: SetReferences does not remove the href"
    );
    let mut doc = edit_case();
    let error = refuse(
        &mut doc,
        SetReferences::new("b", ReferenceEnd::ParticipantPackage, ["i"]),
    );
    assert!(
        matches!(
            error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{ROW}: SetReferences of [i], which removes the href: {error:?}"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_xmi_027_delete_copy_and_move_keep_hrefs() {
    // Scrubbing i from ids [i, j] and the href: b keeps [j] and the href.
    let mut doc = read_ok(
        "ids [i, j] and an href",
        &with_b("", &participants(&["i", "j"], &["o.xmi#p"])),
    );
    let mutation = applied(&mut doc, scrubbing("i"));
    assert_eq!(
        (
            strs(mutation.deleted_ids()),
            strs(element(&doc, "b").participant_packages())
        ),
        (vec!["i"], vec!["j"]),
        "{ROW}: scrubbing i leaves b with j"
    );
    assert_eq!(
        hrefs(&doc, "b", ReferenceEnd::ParticipantPackage),
        ["o.xmi#p"],
        "{ROW}: scrubbing i removes the href"
    );
    // From ids [i] and the href: b is left with one, so it goes (LIB-EDIT-019).
    let mut doc = read_ok(
        "ids [i] and an href",
        &with_b("", &participants(&["i"], &["o.xmi#p"])),
    );
    let mutation = applied(&mut doc, scrubbing("i"));
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["i", "b"],
        "{ROW}: b, left with the href alone, does not go with i"
    );
    // A repeated href is one value: from ids [i] and hrefs [o.xmi#p,
    // o.xmi#p], b is left with one, so it goes.
    let mut doc = read_ok(
        "ids [i] and hrefs [o.xmi#p, o.xmi#p]",
        &with_b("", &participants(&["i"], &["o.xmi#p", "o.xmi#p"])),
    );
    let mutation = applied(&mut doc, scrubbing("i"));
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["i", "b"],
        "{ROW}: b, left with one href held twice, does not go with i"
    );
    // The control: two distinct hrefs are two values, so b stays.
    let mut doc = read_ok(
        "ids [i] and hrefs [o.xmi#p, o.xmi#q]",
        &with_b("", &participants(&["i"], &["o.xmi#p", "o.xmi#q"])),
    );
    let mutation = applied(&mut doc, scrubbing("i"));
    assert_eq!(
        (
            strs(mutation.deleted_ids()),
            strs(element(&doc, "b").participant_packages())
        ),
        (vec!["i"], Vec::<&str>::new()),
        "{ROW}: b, left with two distinct hrefs, does not stay with no ids"
    );
    assert_eq!(
        hrefs(&doc, "b", ReferenceEnd::ParticipantPackage),
        ["o.xmi#p", "o.xmi#q"],
        "{ROW}: scrubbing i changes the two hrefs of b"
    );
    // CopyElements keeps the hrefs.
    let mut doc = read_ok(
        "ids [i, j] and an href",
        &with_b("", &participants(&["i", "j"], &["o.xmi#p"])),
    );
    let copy = CopyElements::new(&doc, ["b"], "case")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy of b: {error}"));
    let mutation = applied(&mut doc, copy);
    let Some(copied) = mutation
        .created_ids()
        .first()
        .map(|id| id.as_str().to_owned())
    else {
        panic!("{ROW}: the copy created nothing");
    };
    assert_eq!(
        strs(element(&doc, &copied).participant_packages()),
        ["i", "j"],
        "{ROW}: the copy's ids"
    );
    assert_eq!(
        hrefs(&doc, &copied, ReferenceEnd::ParticipantPackage),
        ["o.xmi#p"],
        "{ROW}: the copy does not keep the href"
    );
    // A binding of the href alone is not copied.
    let mut lone = read_ok(
        "the href alone",
        &with_b("", &participants(&[], &["o.xmi#p"])),
    );
    let copy = CopyElements::new(&lone, ["b"], "case")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy of b: {error}"));
    let error = refuse(&mut lone, copy);
    assert!(
        matches!(
            error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{ROW}: the copy of a binding of one href: {error:?}"
    );
    // Nor is a binding of one href held twice.
    let mut twice = read_ok(
        "one href held twice",
        &with_b("", &participants(&[], &["o.xmi#p", "o.xmi#p"])),
    );
    let copy = CopyElements::new(&twice, ["b"], "case")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy of b: {error}"));
    let error = refuse(&mut twice, copy);
    assert!(
        matches!(
            error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{ROW}: the copy of a binding of one href held twice: {error:?}"
    );
    // MoveElement changes none.
    let mut doc = read_ok(
        "ids [i, j] and an href",
        &with_b("", &participants(&["i", "j"], &["o.xmi#p"])),
    );
    applied(&mut doc, MoveElement::new("b", "p"));
    assert_eq!(
        (
            element(&doc, "b")
                .parent()
                .map(|parent| parent.id().as_str().to_owned()),
            strs(element(&doc, "b").participant_packages())
        ),
        (Some("p".to_owned()), vec!["i", "j"]),
        "{ROW}: the move of b"
    );
    assert_eq!(
        hrefs(&doc, "b", ReferenceEnd::ParticipantPackage),
        ["o.xmi#p"],
        "{ROW}: the move changes the href"
    );
}

// =================================================================================
// What SACM23-XMI-029 reads
// =================================================================================

/// What a link SACM23-XMI-029 reads is held as.
enum Held {
    /// A proxy of `p`: its role and its href.
    Proxy(&'static str, &'static str),
    /// The expression href of the name of `c1`.
    Expression(&'static str),
    /// The one href of the `citedElement` of `c1`.
    Cited(&'static str),
    /// Not held: the reference element is kept whole in `c1`, with one
    /// SACM-XMI-003.
    Kept,
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_027_the_links_sacm23_xmi_029_reads_are_not_plain_hrefs() {
    let inputs = [
        (
            "an untyped argumentElement href",
            in_p("<argumentElement href=\"o.xmi#c\"/>"),
            Held::Proxy("argumentElement", "o.xmi#c"),
        ),
        (
            "a typed argumentElement href",
            in_p("<argumentElement xmi:type=\"sacm:Claim\" href=\"o.xmi#c\"/>"),
            Held::Proxy("argumentElement", "sacm:Claim o.xmi#c"),
        ),
        (
            "an expression href in a name",
            in_p(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                 <name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\">\
                 <expression href=\"o.xmi#t\"/></name></argumentElement>",
            ),
            Held::Expression("o.xmi#t"),
        ),
        (
            "an XPointer href",
            in_p(&claim(
                "c1",
                " isCitation=\"true\"",
                "<citedElement href=\"#xpointer(id('c'))\"/>",
            )),
            Held::Cited("#xpointer(id('c'))"),
        ),
        (
            "a cross-document XPointer href",
            in_p(&claim(
                "c1",
                " isCitation=\"true\"",
                "<citedElement href=\"o.xmi#xpointer(id('c'))\"/>",
            )),
            Held::Cited("o.xmi#xpointer(id('c'))"),
        ),
        (
            "an xlink:href on a reference element",
            in_p(&claim(
                "c1",
                " isCitation=\"true\"",
                "<citedElement xlink:href=\"o.xmi#c\"/>",
            )),
            Held::Kept,
        ),
    ];
    // None is `Unsupported`, and none is SACM-XMI-007: each is held as
    // SACM23-XMI-029 states, in both modes.
    for (what, input, held) in inputs {
        for mode in MODES {
            let output = match read(mode, &input) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error}"),
            };
            assert!(
                output
                    .diagnostics()
                    .iter()
                    .all(|one| one.code() != Code::XMI_EXTERNAL_HREF),
                "{ROW}: {what}: SACM-XMI-007 in {mode:?} mode"
            );
            let doc = output.document();
            let c1 = doc.get("c1");
            let kept = doc.elements().any(SacmElement::has_preserved_content);
            match held {
                Held::Proxy(role, href) => assert_eq!(
                    (
                        doc.get("p").map(|p| p
                            .proxies()
                            .iter()
                            .map(|proxy| (proxy.role(), proxy.href().as_str()))
                            .collect::<Vec<_>>()),
                        output.diagnostics().len(),
                        kept
                    ),
                    (Some(vec![(role, href)]), 0, false),
                    "{ROW}: {what}, {mode:?}: not held as a proxy of p, with no diagnostic"
                ),
                Held::Expression(href) => assert_eq!(
                    (
                        c1.and_then(SacmElement::name)
                            .and_then(|name| name.expression_href())
                            .map(Href::as_str),
                        output.diagnostics().len(),
                        kept
                    ),
                    (Some(href), 0, false),
                    "{ROW}: {what}, {mode:?}: not held as the name's expression href"
                ),
                Held::Cited(href) => assert_eq!(
                    (
                        c1.map(|c1| c1
                            .hrefs(ReferenceEnd::CitedElement)
                            .iter()
                            .map(Href::as_str)
                            .collect::<Vec<_>>()),
                        output.diagnostics().len(),
                        kept
                    ),
                    (Some(vec![href]), 0, false),
                    "{ROW}: {what}, {mode:?}: not held as an href of citedElement"
                ),
                Held::Kept => {
                    let severity = if mode == Mode::Strict {
                        Severity::Error
                    } else {
                        Severity::Warning
                    };
                    assert_eq!(
                        output
                            .diagnostics()
                            .iter()
                            .map(|one| (one.code(), one.severity(), one.template()))
                            .collect::<Vec<_>>(),
                        [(
                            Code::XMI_UNKNOWN_CONTENT,
                            severity,
                            "SACM-XMI-003/element-kept"
                        )],
                        "{ROW}: {what}, {mode:?}: the reference element is not kept with one \
                         SACM-XMI-003"
                    );
                    assert!(
                        c1.is_some_and(|c1| c1.has_preserved_content()
                            && c1.hrefs(ReferenceEnd::CitedElement).is_empty()
                            && c1.cited_element().is_none()),
                        "{ROW}: {what}, {mode:?}: it is not kept in c1, or it is also read"
                    );
                }
            }
        }
    }
}

// =================================================================================
// The catalog
// =================================================================================

/// The catalog row of `code`, which must be exactly one.
fn catalog_row(code: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let cell = format!("| {code} |");
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with(&cell))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: docs/sacm/diagnostics.md has {} rows for {code}, not one",
            rows.len()
        );
    };
    (*line).to_owned()
}

#[test]
fn sacm23_xmi_027_the_catalog_rows_describe_hrefs() {
    let line = catalog_row("SACM-XMI-007");
    for part in [
        "SACM23-XMI-029",
        "SACM23-XMI-027",
        "XPointer",
        "`XLink`",
        "`expression`",
        "proxy",
        "`Code::XMI_EXTERNAL_HREF`",
    ] {
        assert!(
            line.contains(part),
            "{ROW}: the catalog's SACM-XMI-007 row does not name {part:?}: {line}"
        );
    }
    for code in ["SACM-REF-001", "SACM-REF-002", "SACM-REF-003"] {
        let line = catalog_row(code);
        assert!(
            line.contains("A cross-document `href` (SACM23-XMI-027) is never this finding."),
            "{ROW}: the catalog's {code} row does not exclude an href: {line}"
        );
    }
    let line = catalog_row("SACM-MULT-001");
    assert!(
        line.contains("an `href` counts as a value of its end (SACM23-XMI-027)"),
        "{ROW}: the catalog's SACM-MULT-001 row does not count an href: {line}"
    );
}
