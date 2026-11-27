//! Tests for the content of a Difference's `target`, which is outside the id
//! space (SACM23-XMI-031; XMI 2.5.1 XMI.xsd, complexType `Difference`,
//! element `target`: `<xsd:any processContents="skip"/>` and
//! `<xsd:anyAttribute processContents="skip"/>`; XML Schema Part 1 §3.10.1
//! "skip: No constraints at all: the item must simply be well-formed XML",
//! §3.15.5; SACM 2.3 §2.4; ADR 0042 amendment 2).
//!
//! The attributes of an unqualified `target` child of a Difference
//! (`xmi:Difference`, `xmi:difference`, `xmi:Add`, `xmi:Replace`,
//! `xmi:Delete`, or an unqualified `difference` or `container` in one), and
//! all it holds, are kept as written in both modes with no diagnostic,
//! wherever the Difference stands. Nothing there is an id: no repeat, empty
//! id or non-NCName is refused by a read or a strict write, `is_id_in_use` is
//! false for it, `with_id` may take it, and it steers no generated id; a
//! reference naming only such an `xmi:id` dangles. An `xmi:id` repeated
//! outside a target is still refused, naming only the outside locations.
//!
//! Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{ArgumentValue, Code, Diagnostic, Severity};
use sacm::edit::{self, CopyElements, CreateClaim, EditError};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "SACM23-XMI-031";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The fixture of the design brief and its output in both modes.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/difference-target-ids-valid.sacm.xmi"
);
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/difference-target-ids.golden.sacm.xmi"
);

// --- Inputs ----------------------------------------------------------------------

fn file(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"))
}

/// A Claim `id` with the extra attributes `attributes`, holding `body` after
/// its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// An `xmi:XMI` holding `before`, then the root ArgumentPackage `p`, which
/// holds `in_p` after its name.
fn document(before: &str, in_p: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">{before}\
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/>{in_p}</sacm:ArgumentPackage>\
         </xmi:XMI>"
    )
}

// --- Reading and writing ---------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in `mode`, which must return a document.
fn read_ok(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?} ({error})"),
    }
}

/// Reads `input` in `mode`, which must return a document with no
/// diagnostic.
fn read_clean(what: &str, mode: Mode, input: &str) -> Document {
    let output = read_ok(what, mode, input);
    assert_eq!(
        output.diagnostics(),
        [],
        "{ROW}: {what}: a {mode:?} read gives diagnostics"
    );
    output.into_document()
}

/// The refusal of `input` read in `mode`.
fn refusal(what: &str, mode: Mode, input: &str) -> ReadError {
    match read(mode, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a {mode:?} read returned a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// Asserts that `input` is refused in both modes with `DuplicateId` for
/// `id`, SACM-ID-001.
fn assert_duplicate_in_both_modes(what: &str, input: &str, id: &str) {
    for mode in MODES {
        let error = refusal(what, mode, input);
        assert!(
            matches!(&error, ReadError::DuplicateId { id: found, .. } if found.as_str() == id),
            "{ROW}: {what}: a {mode:?} read is not DuplicateId {id:?}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::ID_DUPLICATE,
            "{ROW}: {what}: a {mode:?} refusal's code() is not SACM-ID-001"
        );
    }
}

fn write(doc: &Document, mode: Mode) -> Result<String, WriteError> {
    xmi::write(doc, &WriteOptions::default().with_mode(mode))
}

/// `doc` written in `mode`, which must succeed.
fn written(what: &str, doc: &Document, mode: Mode) -> String {
    match write(doc, mode) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused it: {error:?} ({error})"),
    }
}

/// The diagnostics of the strict write refusal of `doc`, which must be
/// `WriteError::Invalid`.
fn invalid_diagnostics(what: &str, doc: &Document) -> Vec<Diagnostic> {
    match write(doc, Mode::Strict) {
        Err(error @ WriteError::Invalid { .. }) => error.diagnostics().to_vec(),
        Err(error) => panic!("{ROW}: {what}: the strict write is not Invalid: {error:?}"),
        Ok(text) => panic!("{ROW}: {what}: the strict write writes it:\n{text}"),
    }
}

/// The element `id` of `doc`.
fn element<'d>(what: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    doc.get(id)
        .unwrap_or_else(|| panic!("{ROW}: {what}: the document has no element {id:?}"))
}

/// Applies `operation` to `doc`, which must succeed.
fn apply(what: &str, doc: &mut Document, operation: impl Into<edit::Operation>) -> edit::Mutation {
    match edit::apply(doc, operation) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {what}: the edit was refused: {error:?} ({error})"),
    }
}

/// The ids `mutation` created, as text.
fn created(mutation: &edit::Mutation) -> Vec<&str> {
    mutation
        .created_ids()
        .iter()
        .map(ElementId::as_str)
        .collect()
}

/// The severity of SACM-XMI-003 in `mode`: an Error strict, a Warning
/// tolerant.
fn kept_severity(mode: Mode) -> Severity {
    if mode == Mode::Strict {
        Severity::Error
    } else {
        Severity::Warning
    }
}

/// The arguments of a diagnostic, by name, each as its text or count.
fn arguments(found: &Diagnostic) -> Vec<(&'static str, String)> {
    found
        .arguments()
        .iter()
        .map(|argument| {
            let value = match argument.value() {
                ArgumentValue::Text(text) => text.clone(),
                ArgumentValue::Count(count) => count.to_string(),
                other => format!("{other:?}"),
            };
            (argument.name(), value)
        })
        .collect()
}

/// What a diagnostic is: its code, severity, template and affected ids.
type Summary = (&'static str, Severity, &'static str, Vec<String>);

fn summary(found: &Diagnostic) -> Summary {
    (
        found.code().as_str(),
        found.severity(),
        found.template(),
        found
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    )
}

// --- 1. The fixture reads in both modes --------------------------------------------

#[test]
fn sacm23_xmi_031_the_fixture_reads_in_both_modes_with_no_diagnostic() {
    let input = file(FIXTURE);
    let strict = read_clean("the fixture", Mode::Strict, &input);
    let tolerant = read_clean("the fixture", Mode::Tolerant, &input);
    for (mode, doc) in [(Mode::Strict, &strict), (Mode::Tolerant, &tolerant)] {
        let what = format!("the fixture, read {mode:?}");
        assert!(
            !doc.has_preserved_content(),
            "{ROW}: {what}: the document holds preserved content"
        );
        assert_eq!(
            doc.element_count(),
            2,
            "{ROW}: {what}: not two elements, p and c1"
        );
        let c1 = element(&what, doc, "c1");
        assert_eq!(
            (c1.kind(), c1.name().and_then(LangString::content)),
            (ElementKind::Claim, Some("Safe")),
            "{ROW}: {what}: c1 is not the Claim named \"Safe\""
        );
    }
    assert!(
        strict == tolerant,
        "{ROW}: the fixture: the strict and the tolerant documents are not =="
    );
}

// --- 2. Both writes give the golden output -----------------------------------------

#[test]
fn sacm23_xmi_031_both_writes_give_the_golden_output() {
    let input = file(FIXTURE);
    let golden = file(GOLDEN);
    for read_mode in MODES {
        let doc = read_clean("the fixture", read_mode, &input);
        for write_mode in MODES {
            assert_eq!(
                written("the fixture", &doc, write_mode),
                golden,
                "{ROW}: the fixture, read {read_mode:?}: a {write_mode:?} write is not the golden \
                 output"
            );
        }
        let back = read_clean("the golden output", Mode::Strict, &golden);
        assert_eq!(
            semantic_compare(&doc, &back),
            [],
            "{ROW}: the fixture, read {read_mode:?}: the golden output read back differs"
        );
        assert!(
            back == doc,
            "{ROW}: the fixture, read {read_mode:?}: the golden output read back is not =="
        );
        assert_eq!(
            written("the golden output", &back, Mode::Strict),
            golden,
            "{ROW}: the fixture, read {read_mode:?}: a second write is not byte-stable"
        );
    }
}

// --- 3. No xmi:id in a target is in the id space -----------------------------------

#[test]
fn sacm23_xmi_031_no_xmi_id_in_a_target_is_in_the_id_space() {
    let mut doc = read_clean("the fixture", Mode::Strict, &file(FIXTURE));
    for id in ["p", "c1", "d1", "d2"] {
        assert!(
            doc.is_id_in_use(id),
            "{ROW}: the fixture: is_id_in_use({id:?}) is false"
        );
    }
    for id in ["1x", "t1", "", " a b ", "a b", "x"] {
        assert!(
            !doc.is_id_in_use(id),
            "{ROW}: the fixture: is_id_in_use({id:?}) is true, but only a target holds it"
        );
    }
    let mutation = apply(
        "CreateClaim with the id t1, which only a target holds",
        &mut doc,
        CreateClaim::new("p", LangString::new("T")).with_id("t1"),
    );
    assert_eq!(
        created(&mutation),
        ["t1"],
        "{ROW}: CreateClaim::with_id(\"t1\") does not create t1"
    );
    let text = written("the fixture with the Claim t1", &doc, Mode::Strict);
    let back = read_clean(
        "the fixture with the Claim t1, written",
        Mode::Strict,
        &text,
    );
    assert!(
        back == doc,
        "{ROW}: the fixture with the Claim t1: the strict output read back is not ==:\n{text}"
    );
    let answer = edit::apply(
        &mut doc,
        CreateClaim::new("p", LangString::new("D")).with_id("d2"),
    );
    assert!(
        matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "d2"),
        "{ROW}: CreateClaim::with_id(\"d2\"), the id of an xmi:Add, is not DuplicateId d2: \
         {answer:?}"
    );
}

// --- 4. A strict write checks no id in a target ------------------------------------

#[test]
fn sacm23_xmi_031_a_strict_write_checks_no_id_in_a_target() {
    for (what, input, kept) in [
        (
            "a target under xmi:XMI holding a non-NCName and an empty xmi:id",
            document(
                "<xmi:Difference><target xmi:id=\"1x\"><x xmi:id=\"\"/></target></xmi:Difference>",
                "",
            ),
            "<target xmi:id=\"1x\"><x xmi:id=\"\" /></target>",
        ),
        (
            "a target in an element in an Extension in p, holding a non-NCName",
            document(
                "",
                "<xmi:Extension><foo><xmi:Difference><target xmi:id=\"3z\"/></xmi:Difference>\
                 </foo></xmi:Extension>",
            ),
            "<target xmi:id=\"3z\" />",
        ),
    ] {
        let doc = read_clean(what, Mode::Strict, &input);
        let text = written(what, &doc, Mode::Strict);
        assert!(
            text.contains(kept),
            "{ROW}: {what}: the strict write does not write {kept:?} as written:\n{text}"
        );
    }
    // Unchanged: outside a target, metadata ids are still checked.
    for (what, input, element_name, xmi_id, affected) in [
        (
            "<xmi:Difference xmi:id=\"1x\"/> under xmi:XMI",
            document("<xmi:Difference xmi:id=\"1x\"/>", ""),
            "xmi:Difference",
            "1x",
            Vec::new(),
        ),
        (
            "<xmi:Extension><x xmi:id=\"2y\"/></xmi:Extension> in p",
            document("", "<xmi:Extension><x xmi:id=\"2y\"/></xmi:Extension>"),
            "x",
            "2y",
            vec!["p".to_owned()],
        ),
    ] {
        let doc = read_clean(what, Mode::Strict, &input);
        let found = invalid_diagnostics(what, &doc);
        assert_eq!(
            found.iter().map(summary).collect::<Vec<_>>(),
            [(
                "SACM-ID-002",
                Severity::Error,
                "SACM-ID-002/metadata-id-not-an-ncname",
                affected
            )],
            "{ROW}: {what}: not one SACM-ID-002/metadata-id-not-an-ncname"
        );
        assert_eq!(
            arguments(&found[0]),
            [
                ("element", element_name.to_owned()),
                ("xmi_id", xmi_id.to_owned()),
            ],
            "{ROW}: {what}: the arguments"
        );
    }
}

// --- 5. A reference to an xmi:id in a target dangles -------------------------------

#[test]
fn sacm23_xmi_031_a_reference_to_an_xmi_id_in_a_target_dangles() {
    // A metadata IDREFS token naming only the xmi:id of a target.
    let what = "<xmi:Difference target=\"t1\"><target xmi:id=\"t1\"/></xmi:Difference>";
    let doc = read_clean(
        what,
        Mode::Strict,
        &document(
            "<xmi:Difference target=\"t1\"><target xmi:id=\"t1\"/></xmi:Difference>",
            "",
        ),
    );
    let found = invalid_diagnostics(what, &doc);
    let summaries: Vec<(Summary, &str)> = found
        .iter()
        .map(|found| (summary(found), found.requirement_id()))
        .collect();
    assert_eq!(
        summaries,
        [(
            (
                "SACM-REF-001",
                Severity::Error,
                "SACM-REF-001/metadata-dangling",
                vec!["t1".to_owned()]
            ),
            "SACM23-XMI-019"
        )],
        "{ROW}: {what}: not exactly one SACM-REF-001/metadata-dangling"
    );
    assert_eq!(
        arguments(&found[0]),
        [
            ("attribute", "target".to_owned()),
            ("element", "xmi:Difference".to_owned()),
            ("target", "t1".to_owned()),
        ],
        "{ROW}: {what}: the arguments"
    );
    // A typed reference naming only the xmi:id of a target names no element
    // and no metadata: SACM-REF-001/dangling, not SACM-REF-003.
    let what = "a Claim c2 citing t1, which only a target holds";
    let input = document(
        "<xmi:Difference><target xmi:id=\"t1\"/></xmi:Difference>",
        &claim("c2", " isCitation=\"true\" citedElement=\"t1\"", ""),
    );
    let doc = read_ok(what, Mode::Strict, &input).into_document();
    let found = validation::validate(&doc);
    let dangling: Vec<&Diagnostic> = found
        .iter()
        .filter(|found| found.template() == "SACM-REF-001/dangling")
        .collect();
    assert_eq!(
        dangling.len(),
        1,
        "{ROW}: {what}: not one SACM-REF-001/dangling: {found:?}"
    );
    let named = arguments(dangling[0]);
    for (name, value) in [
        ("class", "Claim"),
        ("id", "c2"),
        ("end", "citedElement"),
        ("target", "t1"),
    ] {
        assert!(
            named.iter().any(|(n, v)| *n == name && v == value),
            "{ROW}: {what}: the SACM-REF-001/dangling has no argument {name} = {value:?}: \
             {named:?}"
        );
    }
    assert_eq!(
        (
            dangling[0].code().as_str(),
            dangling[0].severity(),
            dangling[0]
                .affected()
                .iter()
                .map(ElementId::as_str)
                .collect::<Vec<_>>()
        ),
        ("SACM-REF-001", Severity::Error, vec!["c2", "t1"]),
        "{ROW}: {what}: the SACM-REF-001/dangling's code, severity and affected ids"
    );
    assert!(
        found
            .iter()
            .all(|found| found.code().as_str() != "SACM-REF-003"),
        "{ROW}: {what}: validate gives SACM-REF-003, as if t1 were a metadata id: {found:?}"
    );
}

// --- 6. A repeat outside a target still refuses ------------------------------------

#[test]
fn sacm23_xmi_031_a_repeat_outside_a_target_still_refuses_and_names_no_target() {
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\n\
         \x20 <xmi:Delete><target xmi:id=\"c1\"/></xmi:Delete>\n\
         \x20 <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/>\n\
         \x20   <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"A\"/></argumentElement>\n\
         \x20   <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"B\"/></argumentElement>\n\
         \x20 </sacm:ArgumentPackage>\n\
         </xmi:XMI>\n"
    );
    assert!(
        input
            .lines()
            .nth(3)
            .is_some_and(|line| line.starts_with("    <argumentElement")),
        "{ROW}: setup: line 4 is not the first Claim, indented 4 spaces:\n{input}"
    );
    for mode in MODES {
        let what = format!("two Claims c1 beside a target c1, {mode:?}");
        let error = refusal(&what, mode, &input);
        let ReadError::DuplicateId { id, locations, .. } = &error else {
            panic!("{ROW}: {what}: not DuplicateId: {error:?}");
        };
        assert_eq!(
            id.as_str(),
            "c1",
            "{ROW}: {what}: the refusal names another id"
        );
        let at: Vec<(u32, u32)> = locations
            .iter()
            .map(|location| (location.line(), location.column()))
            .collect();
        assert_eq!(
            at,
            [(4, 5), (5, 5)],
            "{ROW}: {what}: the locations are not exactly the two Claims"
        );
        assert_eq!(
            error.locations(),
            locations.as_slice(),
            "{ROW}: {what}: locations() is not the locations field"
        );
        assert_eq!(
            error.code().as_str(),
            "SACM-ID-001",
            "{ROW}: {what}: the refusal's code"
        );
    }
}

// --- 7. Only the target of a Difference is outside the id space --------------------

#[test]
fn sacm23_xmi_031_only_the_target_of_a_difference_is_outside_the_id_space() {
    let c1 = claim("c1", "", "");
    // Still ids.
    for (what, before) in [
        (
            "(a) a target in an Extension under xmi:XMI",
            "<xmi:Extension><target xmi:id=\"c1\"/></xmi:Extension>",
        ),
        (
            "(b) an xmi:target in a Difference",
            "<xmi:Difference><xmi:target xmi:id=\"c1\"/></xmi:Difference>",
        ),
        ("(c) a Difference itself", "<xmi:Difference xmi:id=\"c1\"/>"),
        (
            "(d) a difference in a Difference",
            "<xmi:Difference><difference xmi:id=\"c1\"/></xmi:Difference>",
        ),
    ] {
        assert_duplicate_in_both_modes(what, &document(before, &c1), "c1");
    }
    // Not ids.
    for (what, before) in [
        (
            "(e) a target in a Replace's container",
            "<xmi:Replace><container><target xmi:id=\"c1\"/></container></xmi:Replace>",
        ),
        (
            "(f) a target in an Add's difference",
            "<xmi:Add><difference><target xmi:id=\"c1\"/></difference></xmi:Add>",
        ),
    ] {
        let input = document(before, &c1);
        for mode in MODES {
            read_clean(what, mode, &input);
        }
    }
    // (g) In preserved content: the Difference is SACM-XMI-003 in a Claim,
    // and its target's xmi:id is still none.
    let what = "(g) a preserved Difference in c2 whose target repeats c1";
    let input = document(
        "",
        &format!(
            "{c1}{}",
            claim(
                "c2",
                "",
                "<xmi:Difference><target xmi:id=\"c1\"/></xmi:Difference>"
            )
        ),
    );
    for mode in MODES {
        let output = read_ok(what, mode, &input);
        assert_eq!(
            output.diagnostics().iter().map(summary).collect::<Vec<_>>(),
            [(
                "SACM-XMI-003",
                kept_severity(mode),
                "SACM-XMI-003/element-kept",
                vec!["c2".to_owned()]
            )],
            "{ROW}: {what}: a {mode:?} read does not give one SACM-XMI-003/element-kept"
        );
        assert!(
            element(what, output.document(), "c2").has_preserved_content(),
            "{ROW}: {what}: a {mode:?} read: c2 does not hold the Difference"
        );
    }
}

// --- 8. Edits and generated ids ignore a target ------------------------------------

#[test]
fn sacm23_xmi_031_edits_and_generated_ids_ignore_a_target() {
    // (a) A copy of c1, whose Extension holds a target naming p as its
    // xmi:id, into its own document.
    let what = "(a) a copy of c1 into p";
    let mut doc = read_clean("the fixture", Mode::Strict, &file(FIXTURE));
    let copy = CopyElements::new(&doc, ["c1"], "p")
        .unwrap_or_else(|error| panic!("{ROW}: {what}: CopyElements::new refused: {error:?}"));
    let mutation = apply(what, &mut doc, copy);
    let ids = created(&mutation);
    let [copied] = ids.as_slice() else {
        panic!("{ROW}: {what}: not one created id: {ids:?}");
    };
    assert_ne!(*copied, "c1", "{ROW}: {what}: the copy kept the id c1");
    let found = element(what, &doc, copied);
    assert_eq!(
        (
            found.kind(),
            found.name().and_then(LangString::content),
            found.parent().map(|parent| parent.id().as_str())
        ),
        (ElementKind::Claim, Some("Safe"), Some("p")),
        "{ROW}: {what}: the copy is not a Claim \"Safe\" in p"
    );
    let text = written(what, &doc, Mode::Strict);
    assert_eq!(
        text.matches("<target xmi:id=\"p\" />").count(),
        2,
        "{ROW}: {what}: the strict output does not hold the target twice:\n{text}"
    );
    // (b) Generated ids skip neither the xmi:id of a target nor an href in
    // it.
    let target = "<xmi:Difference><target xmi:id=\"claim_1\" href=\"#claim_2\"/></xmi:Difference>";
    let what = "(b) CreateClaim beside a target claim_1 naming claim_2";
    let mut doc = read_clean(what, Mode::Strict, &document(target, ""));
    for expected in ["claim_1", "claim_2"] {
        let mutation = apply(what, &mut doc, CreateClaim::new("p", LangString::new("G")));
        assert_eq!(
            created(&mutation),
            [expected],
            "{ROW}: {what}: CreateClaim does not create {expected}"
        );
    }
    let text = written(what, &doc, Mode::Strict);
    let back = read_clean(what, Mode::Strict, &text);
    assert!(
        back == doc,
        "{ROW}: {what}: the strict output read back is not ==:\n{text}"
    );
    // (c) A Claim read with no xmi:id beside such a target.
    let input = document(
        target,
        "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>",
    );
    for mode in MODES {
        let what = format!("(c) an id-less Claim beside a target claim_1, {mode:?}");
        let output = read_ok(&what, mode, &input);
        let codes: Vec<&str> = output
            .diagnostics()
            .iter()
            .map(|found| found.code().as_str())
            .collect();
        assert_eq!(
            codes,
            ["SACM-XMI-004"],
            "{ROW}: {what}: not one SACM-XMI-004 for the missing id"
        );
        let claims: Vec<&str> = output
            .document()
            .elements()
            .filter(|found| found.kind() == ElementKind::Claim)
            .map(|found| found.id().as_str())
            .collect();
        assert_eq!(
            claims,
            ["claim_1"],
            "{ROW}: {what}: the Claim's generated id is not claim_1"
        );
    }
}
