//! Foreign subclasses of SACM classes (LIB-COMPAT-017; ADR 0044 D3, D4;
//! D-STRICT).
//!
//! XMI 2.5.1 §7.8.6 (p. 19): "If the org.omg.xmi.allowMetamodelExtension tag
//! is set to true, then the name of the type is replaced by "xmi:Any": this
//! declaration enables any object to be serialized, enhancing the
//! extensibility of models." ptc/22-03-13 sets no `org.omg.xmi` tag, so the
//! tag is false (Annex B.2, p. 104) and a role's candidate types are SACM's
//! own classes: an element typed with a foreign class is not SACM 2.3 XMI.
//! Reading one is a tolerance that the application asks for:
//!
//! - `ReadOptions::with_subclass(namespace, name, kind)` declares the class
//!   `name` of `namespace` a subclass of the SACM 2.3 class `kind`;
//! - a tolerant `xmi::read` then reads a child typed with that class (by
//!   `xmi:type` or `xsi:type`, matched by namespace URI and local name), under
//!   a role that admits `kind`, as an element of `kind`, keeps the class as
//!   preserved content (`SacmElement::subclass`), and reports one SACM-XMI-003
//!   Warning `subclass-type-kept` per element;
//! - a root keeps the type of a declared subclass of its own class (slice
//!   0075b Q1 = (a), the maintainer, 2026-11-24); its class still comes from
//!   its element name (D-ROOT-NAME);
//! - a tolerant write writes the class back as the element's `xmi:type`; a
//!   strict write refuses the element (SACM-XMI-006);
//! - a declaration never applies to a proxy, nor to an element that carries
//!   a type in both XMI namespaces, nor to a type in the root's SACM
//!   namespaces or in SACM 2.3's, and a strict read ignores every
//!   declaration.
//!
//! G3 is `tests/data/compat/subclasses.xmi` and G4 its tolerant write with
//! the declarations, `tests/data/compat/golden/subclasses.golden.sacm.xmi`;
//! G5 is `tests/data/compat/emf-subclasses.xmi` and G6 its tolerant write,
//! `tests/data/compat/golden/emf-subclasses.golden.sacm.xmi`; J3 is
//! `tests/data/golden/serde/subclasses-xmi-003.json`, c1's Warning from G3 as
//! JSON. None of G3 to G6 is SACM 2.3 XMI: each holds foreign types.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Argument, Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CopyElements, DeleteElement, MoveElement};
use sacm::model::{Document, ElementId, ElementKind, SacmElement, Subclass};
use sacm::validation;
use sacm::xmi::{self, Limits, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "LIB-COMPAT-017";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The XMI namespace EMF writes (`ExtendedMetaData.XMI_URI`).
const EMF_NS: &str = "http://www.omg.org/XMI";

/// The application's profile, whose classes the tests declare subclasses.
const P: &str = "urn:example:profile";

/// The template of the new Warning.
const KEPT: &str = "SACM-XMI-003/subclass-type-kept";
const ELEMENT_KEPT: &str = "SACM-XMI-003/element-kept";
const ATTRIBUTE_KEPT: &str = "SACM-XMI-003/attribute-kept";

/// G3: SACM 2.3 roles typed with the profile's classes.
const G3: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/compat/subclasses.xmi"
);
/// G4: G3's tolerant write, read with [`options`].
const G4: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/compat/golden/subclasses.golden.sacm.xmi"
);
/// G5: the EMF dialect with the profile's classes by `xsi:type`.
const G5: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/compat/emf-subclasses.xmi"
);
/// G6: G5's tolerant write, read with [`options`].
const G6: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/compat/golden/emf-subclasses.golden.sacm.xmi"
);
/// J3: c1's `subclass-type-kept` Warning from G3, as JSON.
#[cfg(feature = "serde")]
const J3: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/golden/serde/subclasses-xmi-003.json"
);

// --- Helpers ---------------------------------------------------------------------

/// The three declarations of the tests, in tolerant mode: `Bundle` a
/// subclass of `ArgumentPackage`, `Proposition` of `Claim` and `Step` of
/// `AssertedInference`.
fn options() -> ReadOptions {
    ReadOptions::default()
        .with_subclass(P, "Bundle", ElementKind::ArgumentPackage)
        .with_subclass(P, "Proposition", ElementKind::Claim)
        .with_subclass(P, "Step", ElementKind::AssertedInference)
}

fn loc(line: u32, column: u32) -> SourceLocation {
    SourceLocation::new(line, column)
        .unwrap_or_else(|| panic!("{ROW}: setup: ({line}, {column}) is no location"))
}

/// The location of the first `needle` in `text`, whose lines end with LF:
/// the line, and the column in characters.
fn position_of(text: &str, needle: &str) -> SourceLocation {
    let Some(offset) = text.find(needle) else {
        panic!("{ROW}: setup: {needle:?} is not in {text}");
    };
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    loc(
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}

/// The text of the file `path`.
fn fixture(path: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"))
}

/// The read of `input` with `options`, which must succeed.
fn read_with(what: &str, input: &str, options: &ReadOptions) -> ReadOutput {
    match xmi::read(input.as_bytes(), options) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: the read refuses it: {error:?} ({error})"),
    }
}

/// The refusal of `input` with `options`.
fn refusal_with(what: &str, input: &str, options: &ReadOptions) -> ReadError {
    match xmi::read(input.as_bytes(), options) {
        Ok(output) => panic!(
            "{ROW}: {what}: the read returns a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// The tolerant write of `document`, which must succeed.
fn tolerant_write(what: &str, document: &Document) -> String {
    xmi::write(document, &WriteOptions::default().with_mode(Mode::Tolerant)).unwrap_or_else(
        |error| panic!("{ROW}: {what}: a tolerant write refuses it: {error:?} ({error})"),
    )
}

/// Asserts that the tolerant write of `document`, read back with
/// `options`, is `==` to it, and gives the text written.
fn assert_writes_back(what: &str, document: &Document, options: &ReadOptions) -> String {
    let text = tolerant_write(what, document);
    let back = read_with(&format!("{what}, written back"), &text, options).into_document();
    assert!(
        back == *document,
        "{ROW}: {what}: the tolerant write does not read back ==: {:?}\n{text}",
        semantic_compare(document, &back)
    );
    text
}

/// The element `id` of `document`, which must exist.
fn element<'a>(what: &str, document: &'a Document, id: &str) -> SacmElement<'a> {
    document
        .get(id)
        .unwrap_or_else(|| panic!("{ROW}: {what}: the document has no element {id:?}"))
}

/// A class as `(namespace, name)`.
type Class = (String, String);

/// The subclass of `element`.
fn class_of(element: SacmElement<'_>) -> Option<Class> {
    element
        .subclass()
        .map(|class| (class.namespace().to_owned(), class.name().to_owned()))
}

/// The class `name` of [`P`].
fn in_profile(name: &str) -> Class {
    (P.to_owned(), name.to_owned())
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

fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
}

/// The diagnostics of `found` with the template `template`.
fn with_template<'a>(found: &'a [Diagnostic], template: &str) -> Vec<&'a Diagnostic> {
    found
        .iter()
        .filter(|diagnostic| diagnostic.template() == template)
        .collect()
}

/// The one diagnostic of `found` with the template `template`.
fn only<'a>(what: &str, found: &'a [Diagnostic], template: &str) -> &'a Diagnostic {
    let all = with_template(found, template);
    let [one] = all.as_slice() else {
        panic!(
            "{ROW}: {what}: not one {template} but {}, among {}",
            all.len(),
            brief(found)
        );
    };
    one
}

/// `found` as `(template, severity, location)`, for a short failure message.
fn brief(found: &[Diagnostic]) -> String {
    let shown: Vec<String> = found
        .iter()
        .map(|diagnostic| {
            format!(
                "{} {} at {:?}",
                diagnostic.template(),
                diagnostic.severity().as_str(),
                diagnostic.location().map(|at| (at.line(), at.column()))
            )
        })
        .collect();
    format!("{shown:#?}")
}

/// The diagnostics as a count per `(code, severity, template)`.
fn tally(found: &[Diagnostic]) -> BTreeMap<(String, &'static str, &'static str), usize> {
    let mut counts = BTreeMap::new();
    for diagnostic in found {
        *counts
            .entry((
                diagnostic.code().as_str().to_owned(),
                diagnostic.severity().as_str(),
                diagnostic.template(),
            ))
            .or_insert(0) += 1;
    }
    counts
}

fn expected_tally(
    rows: &[(&str, &'static str, &'static str, usize)],
) -> BTreeMap<(String, &'static str, &'static str), usize> {
    rows.iter()
        .map(|(code, severity, template, count)| {
            (((*code).to_owned(), *severity, *template), *count)
        })
        .collect()
}

/// The locations of the diagnostics of `found` with `template`, in order.
fn locations(found: &[Diagnostic], template: &str) -> Vec<Option<SourceLocation>> {
    with_template(found, template)
        .iter()
        .map(|diagnostic| diagnostic.location())
        .collect()
}

/// Asserts that `diagnostic` is a SACM-XMI-003 Warning `subclass-type-kept`
/// of LIB-COMPAT-017, clause 7.8.6, at `at`, affecting `id`, with the
/// arguments `type`, `class` and `id`.
fn assert_subclass_kept(
    what: &str,
    diagnostic: &Diagnostic,
    at: SourceLocation,
    written_type: &str,
    class: &str,
    id: &str,
) {
    assert_eq!(
        (
            diagnostic.code(),
            diagnostic.severity(),
            diagnostic.template(),
            diagnostic.requirement_id(),
            diagnostic.clause(),
            diagnostic.location(),
            strs(diagnostic.affected()),
            arguments_of(diagnostic.arguments()),
        ),
        (
            Code::XMI_UNKNOWN_CONTENT,
            Severity::Warning,
            KEPT,
            ROW,
            Some("7.8.6"),
            Some(at),
            vec![id],
            owned(&[("type", written_type), ("class", class), ("id", id)]),
        ),
        "{ROW}: {what}: the code, severity, template, row, clause, location, affected ids and \
         arguments of the subclass-type-kept Warning"
    );
}

/// Asserts that `diagnostic` is a SACM-XMI-003 `element-kept` of the
/// `element` at `at` in `holder_class` `holder_id`, with `severity`.
fn assert_element_kept(
    what: &str,
    diagnostic: &Diagnostic,
    severity: Severity,
    at: SourceLocation,
    holder: (&str, &str),
) {
    assert_eq!(
        (
            diagnostic.code(),
            diagnostic.severity(),
            diagnostic.template(),
            diagnostic.location(),
            strs(diagnostic.affected()),
            arguments_of(diagnostic.arguments()),
        ),
        (
            Code::XMI_UNKNOWN_CONTENT,
            severity,
            ELEMENT_KEPT,
            Some(at),
            vec![holder.1],
            owned(&[
                ("element", "argumentElement"),
                ("holder_class", holder.0),
                ("holder_id", holder.1)
            ]),
        ),
        "{ROW}: {what}: the code, severity, template, location, affected ids and arguments of \
         the element-kept"
    );
}

/// The ArgumentPackage `a`, with `child` on line 3 from column 3. It binds
/// `sacm`, `xmi`, `xsi`, and `p` and `q` both to [`P`].
fn doc(child: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:p=\"{P}\" \
         xmlns:q=\"{P}\" xmi:id=\"a\">\n  <name lang=\"en\" content=\"a\"/>\n  {child}\n\
         </sacm:ArgumentPackage>"
    )
}

/// The argumentElement `c1` with `attributes`, whose first attribute is at
/// (3,20) in [`doc`].
fn child(attributes: &str) -> String {
    format!(
        "<argumentElement {attributes} xmi:id=\"c1\"><name lang=\"en\" content=\"c\"/>\
         </argumentElement>"
    )
}

/// The root ArgumentPackage `a`, typed `xmi:type`, binding `p` to [`P`].
fn typed_root(xmi_type: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:p=\"{P}\" xmi:type=\"{xmi_type}\" xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>\
         </sacm:ArgumentPackage>"
    )
}

/// G3 read with [`options`].
fn g3_read() -> ReadOutput {
    read_with("G3", &fixture(G3), &options())
}

/// The elements of `document` that have a `subclass()`, by id.
fn with_subclass(document: &Document) -> Vec<String> {
    document
        .elements()
        .filter(|element| element.subclass().is_some())
        .map(|element| element.id().as_str().to_owned())
        .collect()
}

// --- 1. G3 and G4 ----------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one read of G3 checked against every clause of acceptance 1, in order"
)]
fn lib_compat_017_declared_subclasses_read_as_their_sacm_classes_and_write_back_their_golden() {
    let output = g3_read();
    let document = output.document();

    // Kinds.
    let kinds: Vec<(&str, ElementKind)> = document
        .elements()
        .map(|element| (element.id().as_str(), element.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("case", ElementKind::AssuranceCasePackage),
            ("arguments", ElementKind::ArgumentPackage),
            ("c1", ElementKind::Claim),
            ("c2", ElementKind::Claim),
            ("i1", ElementKind::AssertedInference),
        ],
        "{ROW}: G3: the elements and their kinds"
    );

    // `subclass()` and `has_preserved_content()`.
    for (id, class, holds) in [
        ("case", None, false),
        ("arguments", Some(in_profile("Bundle")), true),
        ("c1", Some(in_profile("Proposition")), true),
        ("c2", None, false),
        ("i1", Some(in_profile("Step")), true),
    ] {
        let found = element("G3", document, id);
        assert_eq!(
            (class_of(found), found.has_preserved_content()),
            (class, holds),
            "{ROW}: G3: the subclass() and has_preserved_content() of {id:?}"
        );
    }

    // References.
    let inference = element("G3", document, "i1");
    assert_eq!(
        (strs(inference.sources()), strs(inference.targets())),
        (vec!["c2"], vec!["c1"]),
        "{ROW}: G3: the sources and targets of i1"
    );

    // Diagnostics, exactly, in order.
    let found = output.diagnostics();
    let templates: Vec<&str> = found.iter().map(Diagnostic::template).collect();
    assert_eq!(
        templates,
        [KEPT, KEPT, ATTRIBUTE_KEPT, KEPT],
        "{ROW}: G3: the diagnostics of the read, in order: {}",
        brief(found)
    );
    assert_subclass_kept(
        "G3 arguments",
        &found[0],
        loc(5, 20),
        "p:Bundle",
        "ArgumentPackage",
        "arguments",
    );
    assert_subclass_kept(
        "G3 c1",
        &found[1],
        loc(7, 22),
        "p:Proposition",
        "Claim",
        "c1",
    );
    let attribute = &found[2];
    assert_eq!(
        (
            attribute.code(),
            attribute.severity(),
            attribute.location(),
            strs(attribute.affected()),
            arguments_of(attribute.arguments()),
        ),
        (
            Code::XMI_UNKNOWN_CONTENT,
            Severity::Warning,
            Some(loc(7, 59)),
            vec!["c1"],
            owned(&[
                ("attribute", "p:weight"),
                ("holder_class", "Claim"),
                ("holder_id", "c1")
            ]),
        ),
        "{ROW}: G3: the attribute-kept of p:weight"
    );
    assert_subclass_kept(
        "G3 i1",
        &found[3],
        loc(13, 22),
        "p:Step",
        "AssertedInference",
        "i1",
    );

    // Validation.
    let findings = validation::validate(document);
    assert!(
        findings.is_empty(),
        "{ROW}: G3: validate finds something: {findings:#?}"
    );

    // Write-back: G4, which reads back == and re-writes to G4.
    let g4 = fixture(G4);
    let text = tolerant_write("G3", document);
    assert_eq!(
        text, g4,
        "{ROW}: G3: the tolerant write is not G4 byte for byte"
    );
    let back = read_with("G4", &g4, &options()).into_document();
    assert!(
        back == *document,
        "{ROW}: G4: read with the declarations, it is not == the read of G3: {:?}",
        semantic_compare(document, &back)
    );
    assert_eq!(
        tolerant_write("G4", &back),
        g4,
        "{ROW}: G4: re-written, it is not G4 byte for byte"
    );
}

// --- 2. A strict read ------------------------------------------------------------

#[test]
fn lib_compat_017_a_strict_read_ignores_the_declarations() {
    let g3 = fixture(G3);
    let with = read_with(
        "G3 strict, declared",
        &g3,
        &options().with_mode(Mode::Strict),
    );
    let without = read_with(
        "G3 strict",
        &g3,
        &ReadOptions::default().with_mode(Mode::Strict),
    );
    assert!(
        with.document() == without.document(),
        "{ROW}: G3: a strict read with the declarations is not == one without: {:?}",
        semantic_compare(without.document(), with.document())
    );
    assert_eq!(
        with.diagnostics(),
        without.diagnostics(),
        "{ROW}: G3: a strict read with the declarations reports what one without does not"
    );
    let [kept] = with.diagnostics() else {
        panic!(
            "{ROW}: G3: a strict read does not report exactly one diagnostic: {}",
            brief(with.diagnostics())
        );
    };
    assert_eq!(
        (
            kept.code(),
            kept.severity(),
            kept.template(),
            kept.location(),
            arguments_of(kept.arguments()),
        ),
        (
            Code::XMI_UNKNOWN_CONTENT,
            Severity::Error,
            ELEMENT_KEPT,
            Some(loc(5, 3)),
            owned(&[
                ("element", "argumentPackage"),
                ("holder_class", "AssuranceCasePackage"),
                ("holder_id", "case")
            ]),
        ),
        "{ROW}: G3: the strict read's one Error"
    );
    assert!(
        with_subclass(with.document()).is_empty(),
        "{ROW}: G3: a strict read gives a subclass()"
    );
}

// --- 3. Without a declaration --------------------------------------------------

#[test]
fn lib_compat_017_without_a_declaration_the_element_is_kept_whole() {
    let output = read_with("G3 default", &fixture(G3), &ReadOptions::default());
    let [kept] = output.diagnostics() else {
        panic!(
            "{ROW}: G3: a tolerant read with default options does not report exactly one \
             diagnostic: {}",
            brief(output.diagnostics())
        );
    };
    assert_eq!(
        (
            kept.severity(),
            kept.template(),
            kept.location(),
            arguments_of(kept.arguments())
        ),
        (
            Severity::Warning,
            ELEMENT_KEPT,
            Some(loc(5, 3)),
            owned(&[
                ("element", "argumentPackage"),
                ("holder_class", "AssuranceCasePackage"),
                ("holder_id", "case")
            ]),
        ),
        "{ROW}: G3: the one Warning of a tolerant read with default options"
    );
    let ids: Vec<&str> = output
        .document()
        .elements()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(
        ids,
        ["case"],
        "{ROW}: G3: the elements read without a declaration"
    );
    assert!(
        with_subclass(output.document()).is_empty(),
        "{ROW}: G3: a read without declarations gives a subclass()"
    );
}

// --- 4. A strict write -----------------------------------------------------------

#[test]
fn lib_compat_017_a_strict_write_refuses_a_subclass_element_as_preserved_content() {
    let output = g3_read();
    let error = match xmi::write(output.document(), &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: G3: a strict write writes it:\n{text}"),
        Err(error) => error,
    };
    let WriteError::PreservedContent { elements, .. } = &error else {
        panic!("{ROW}: G3: the strict write is not PreservedContent: {error:?}");
    };
    assert_eq!(
        strs(elements),
        ["arguments", "c1", "i1"],
        "{ROW}: G3: the elements the strict write names"
    );
    assert_eq!(
        error.code(),
        Code::XMI_STRICT_SAVE_REFUSED,
        "{ROW}: G3: the strict write's code is not SACM-XMI-006"
    );
    assert_eq!(
        error.to_string(),
        "the document cannot be written in strict mode: 3 elements hold preserved content, the \
         first is \"arguments\"",
        "{ROW}: G3: the Display of the strict write's refusal"
    );
}

// --- 5. A role that does not admit the kind, and proxies -------------------------

#[test]
fn lib_compat_017_a_class_its_role_does_not_admit_or_a_proxy_is_kept_whole() {
    // (a) Declared an ArtifactPackage, which an argumentElement does not admit.
    let what = "(a) a kind the role does not admit";
    let declared =
        ReadOptions::default().with_subclass(P, "Proposition", ElementKind::ArtifactPackage);
    let input = doc(&child("xmi:type=\"p:Proposition\""));
    let output = read_with(what, &input, &declared);
    let [kept] = output.diagnostics() else {
        panic!(
            "{ROW}: {what}: not exactly one diagnostic: {}",
            brief(output.diagnostics())
        );
    };
    assert_element_kept(
        what,
        kept,
        Severity::Warning,
        loc(3, 3),
        ("ArgumentPackage", "a"),
    );
    assert!(
        output.document().get("c1").is_none(),
        "{ROW}: {what}: c1 is read as an element"
    );

    // (b) Proxies, by href and by xmi:idref.
    for (what, proxy) in [
        (
            "(b) an href proxy",
            "<argumentElement xmi:type=\"p:Proposition\" href=\"other.sacm#c\"/>",
        ),
        (
            "(b) an xmi:idref proxy",
            "<argumentElement xmi:type=\"p:Proposition\" xmi:idref=\"a\"/>",
        ),
    ] {
        let input = doc(proxy);
        let declared = read_with(what, &input, &options());
        let plain = read_with(what, &input, &ReadOptions::default());
        assert!(
            declared.document() == plain.document(),
            "{ROW}: {what}: the read with the declarations is not == the read without: {:?}",
            semantic_compare(plain.document(), declared.document())
        );
        assert_eq!(
            declared.diagnostics(),
            plain.diagnostics(),
            "{ROW}: {what}: the read with the declarations reports what the read without does not"
        );
        let [kept] = declared.diagnostics() else {
            panic!(
                "{ROW}: {what}: not exactly one diagnostic: {}",
                brief(declared.diagnostics())
            );
        };
        assert_element_kept(
            what,
            kept,
            Severity::Warning,
            loc(3, 3),
            ("ArgumentPackage", "a"),
        );
    }
}

// --- 6. SACM namespaces and roots ------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the six cases (a) to (f) of acceptance 6, in order"
)]
fn lib_compat_017_declarations_never_apply_to_a_sacm_namespace_and_a_root_keeps_only_its_own_subclass()
 {
    // (a) A SACM 2.3 class.
    let what = "(a) a SACM 2.3 class";
    let declared =
        ReadOptions::default().with_subclass(SACM_NS, "Claim", ElementKind::ArgumentReasoning);
    let input = doc(&child("xmi:type=\"sacm:Claim\""));
    let output = read_with(what, &input, &declared);
    let c1 = element(what, output.document(), "c1");
    assert_eq!(
        (c1.kind(), class_of(c1)),
        (ElementKind::Claim, None),
        "{ROW}: {what}: c1 is not a Claim without a subclass()"
    );
    assert!(
        output.diagnostics().is_empty(),
        "{ROW}: {what}: the read reports {}",
        brief(output.diagnostics())
    );
    assert_reads_as_without(what, &input, &declared);

    // (b) A type in SACM 2.3's namespace that names no class.
    let what = "(b) a family type naming no class";
    let declared = ReadOptions::default().with_subclass(SACM_NS, "Goal", ElementKind::Claim);
    let input = doc(&child("xmi:type=\"sacm:Goal\""));
    let output = read_with(what, &input, &declared);
    let [kept] = output.diagnostics() else {
        panic!(
            "{ROW}: {what}: not exactly one diagnostic: {}",
            brief(output.diagnostics())
        );
    };
    assert_element_kept(
        what,
        kept,
        Severity::Warning,
        loc(3, 3),
        ("ArgumentPackage", "a"),
    );
    assert_reads_as_without(what, &input, &declared);

    // (c) An older revision's family, and SACM 2.3's, under a SACM 2.2 root.
    let what = "(c) an older revision's family";
    let declared = ReadOptions::default()
        .with_subclass(
            "http://omg.sacm/2.2/argumentation",
            "Goal",
            ElementKind::Claim,
        )
        .with_subclass(SACM_NS, "Goal", ElementKind::Claim);
    let input = format!(
        "<argumentation:ArgumentPackage xmlns:argumentation=\"http://omg.sacm/2.2/argumentation\" \
         xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\">\n  \
         <argumentElement xmi:type=\"argumentation:Goal\" xmi:id=\"g1\"/>\n  \
         <argumentElement xmi:type=\"sacm:Goal\" xmi:id=\"g2\"/>\n\
         </argumentation:ArgumentPackage>"
    );
    let output = read_with(what, &input, &declared);
    let found = output.diagnostics();
    assert_eq!(
        tally(found),
        expected_tally(&[
            ("SACM-XMI-003", "warning", ELEMENT_KEPT, 2),
            ("SACM-XMI-008", "warning", "SACM-XMI-008/other-revision", 1),
        ]),
        "{ROW}: {what}: the diagnostics: {}",
        brief(found)
    );
    assert_eq!(
        (
            locations(found, "SACM-XMI-008/other-revision"),
            locations(found, ELEMENT_KEPT)
        ),
        (
            vec![Some(loc(1, 1))],
            vec![Some(loc(2, 3)), Some(loc(3, 3))]
        ),
        "{ROW}: {what}: the locations of SACM-XMI-008 and of the element-kept Warnings"
    );
    assert_reads_as_without(what, &input, &declared);

    // (d) A document element in a foreign namespace, in both modes.
    let what = "(d) a foreign document element";
    let input = format!("<p:Bundle xmlns:p=\"{P}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/>");
    for mode in [Mode::Strict, Mode::Tolerant] {
        let error = refusal_with(what, &input, &options().with_mode(mode));
        assert!(
            matches!(error, ReadError::InvalidRoot { .. }),
            "{ROW}: {what}: a {mode:?} read is not InvalidRoot: {error:?}"
        );
        assert_eq!(
            (error.template(), error.location()),
            ("SACM-XMI-001/not-a-package", Some(loc(1, 1))),
            "{ROW}: {what}: the template and location of a {mode:?} read's refusal"
        );
        assert_eq!(
            error,
            refusal_with(what, &input, &ReadOptions::default().with_mode(mode)),
            "{ROW}: {what}: a {mode:?} read with the declarations refuses it otherwise than one \
             without"
        );
    }

    // (e) A root typed with a declared subclass of its own class (Q1 = (a)).
    let what = "(e) a root typed with a subclass of its own class";
    let input = typed_root("p:Bundle");
    let output = read_with(what, &input, &options());
    let root = element(what, output.document(), "a");
    assert_eq!(
        (root.kind(), class_of(root)),
        (ElementKind::ArgumentPackage, Some(in_profile("Bundle"))),
        "{ROW}: {what}: a is not an ArgumentPackage with the subclass Bundle"
    );
    let [kept] = output.diagnostics() else {
        panic!(
            "{ROW}: {what}: not exactly one diagnostic: {}",
            brief(output.diagnostics())
        );
    };
    assert_subclass_kept(
        what,
        kept,
        position_of(&input, "xmi:type="),
        "p:Bundle",
        "ArgumentPackage",
        "a",
    );
    let text = assert_writes_back(what, output.document(), &options());
    assert!(
        text.contains(&format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
             xmi:type=\"p:Bundle\" xmi:id=\"a\" xmlns:p=\"{P}\">"
        )),
        "{ROW}: {what}: the tolerant write's start tag is not the type's:\n{text}"
    );

    // (f) A root typed with a declared subclass of another kind.
    let what = "(f) a root typed with a subclass of another kind";
    let input = typed_root("p:Proposition");
    let output = read_with(what, &input, &options());
    let root = element(what, output.document(), "a");
    assert_eq!(
        (root.kind(), class_of(root)),
        (ElementKind::ArgumentPackage, None),
        "{ROW}: {what}: a is not an ArgumentPackage without a subclass()"
    );
    let [kept] = output.diagnostics() else {
        panic!(
            "{ROW}: {what}: not exactly one diagnostic: {}",
            brief(output.diagnostics())
        );
    };
    assert_eq!(
        (
            kept.template(),
            kept.location(),
            arguments_of(kept.arguments())
        ),
        (
            ATTRIBUTE_KEPT,
            Some(position_of(&input, "xmi:type=")),
            owned(&[
                ("attribute", "xmi:type"),
                ("holder_class", "ArgumentPackage"),
                ("holder_id", "a")
            ]),
        ),
        "{ROW}: {what}: the type is not kept as an attribute"
    );
}

/// Asserts that the tolerant read of `input` with `declared` equals the one
/// with default options, in document and diagnostics.
fn assert_reads_as_without(what: &str, input: &str, declared: &ReadOptions) {
    let with = read_with(what, input, declared);
    let without = read_with(what, input, &ReadOptions::default());
    assert!(
        with.document() == without.document(),
        "{ROW}: {what}: the read with the declarations is not == the read without: {:?}",
        semantic_compare(without.document(), with.document())
    );
    assert_eq!(
        with.diagnostics(),
        without.diagnostics(),
        "{ROW}: {what}: the read with the declarations reports otherwise than the read without"
    );
}

// --- 7. Matching by namespace and name -------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the nine cases (a) to (i) of acceptance 7, in order"
)]
fn lib_compat_017_a_type_is_matched_by_namespace_and_name_whatever_its_prefix() {
    /// A case that reads: what it is, the input, the written type, and the
    /// other diagnostics as (template, location).
    type ReadCase = (
        &'static str,
        String,
        &'static str,
        Vec<(&'static str, SourceLocation)>,
    );
    let first = loc(3, 20);
    let emf = doc(&child("xmi:type=\"p:Proposition\"")).replace(XMI_NS, EMF_NS);
    let emf_id = position_of(&emf, "xmi:id=");
    let read_cases: [ReadCase; 5] = [
        (
            "(a) another prefix",
            doc(&child("xmi:type=\"q:Proposition\"")),
            "q:Proposition",
            vec![],
        ),
        (
            "(b) xsi:type",
            doc(&child("xsi:type=\"p:Proposition\"")),
            "p:Proposition",
            vec![("SACM-XMI-010/xsi-type", first)],
        ),
        (
            "(c) white space",
            doc(&child("xmi:type=\" p:Proposition \"")),
            "p:Proposition",
            vec![],
        ),
        (
            "(d) agreeing xmi:type and xsi:type",
            doc(&child(
                "xmi:type=\"p:Proposition\" xsi:type=\"q:Proposition\"",
            )),
            "p:Proposition",
            vec![("SACM-XMI-010/xsi-type", loc(3, 45))],
        ),
        (
            "(e) an EMF document",
            emf,
            "p:Proposition",
            vec![("SACM-XMI-015/emf-xmi-namespace-read", emf_id)],
        ),
    ];
    for (what, input, written_type, others) in &read_cases {
        let output = read_with(what, input, &options());
        let c1 = element(what, output.document(), "c1");
        assert_eq!(
            (c1.kind(), class_of(c1)),
            (ElementKind::Claim, Some(in_profile("Proposition"))),
            "{ROW}: {what}: c1 is not a Claim with the subclass Proposition"
        );
        let found = output.diagnostics();
        assert_subclass_kept(
            what,
            only(what, found, KEPT),
            first,
            written_type,
            "Claim",
            "c1",
        );
        let mut rest: Vec<(&str, Option<SourceLocation>)> = found
            .iter()
            .filter(|diagnostic| diagnostic.template() != KEPT)
            .map(|diagnostic| (diagnostic.template(), diagnostic.location()))
            .collect();
        rest.sort();
        let mut expected: Vec<(&str, Option<SourceLocation>)> = others
            .iter()
            .map(|(template, at)| (*template, Some(*at)))
            .collect();
        expected.sort();
        assert_eq!(
            rest, expected,
            "{ROW}: {what}: the other diagnostics of the read"
        );
        let text = assert_writes_back(what, output.document(), &options());
        let prefix = written_type
            .split_once(':')
            .map_or("", |(prefix, _)| prefix);
        assert!(
            text.contains(&format!("xmi:type=\"{written_type}\""))
                && text.contains(&format!("xmlns:{prefix}=\"{P}\"")),
            "{ROW}: {what}: the tolerant write does not write the type {written_type:?} with \
             its binding:\n{text}"
        );
        assert!(
            !text.contains("xsi:type"),
            "{ROW}: {what}: the tolerant write writes an xsi:type:\n{text}"
        );
    }

    // The cases kept whole.
    for (what, attributes) in [
        (
            "(f) disagreeing subclasses",
            "xmi:type=\"p:Proposition\" xsi:type=\"p:Step\"",
        ),
        (
            "(g) a subclass and a SACM class",
            "xmi:type=\"p:Proposition\" xsi:type=\"sacm:Claim\"",
        ),
        ("(h) an unprefixed type", "xmi:type=\"Proposition\""),
        ("(i) a wrongly cased name", "xmi:type=\"p:proposition\""),
    ] {
        let output = read_with(what, &doc(&child(attributes)), &options());
        let [kept] = output.diagnostics() else {
            panic!(
                "{ROW}: {what}: not exactly one diagnostic: {}",
                brief(output.diagnostics())
            );
        };
        assert_element_kept(
            what,
            kept,
            Severity::Warning,
            loc(3, 3),
            ("ArgumentPackage", "a"),
        );
        assert!(
            output.document().get("c1").is_none(),
            "{ROW}: {what}: c1 is read as an element"
        );
    }
}

// --- 8. The written prefix -------------------------------------------------------

#[test]
fn lib_compat_017_the_written_prefix_is_the_types_unless_the_writer_binds_it_otherwise() {
    let what = "a type prefix sacm bound to the profile";
    let input = doc(
        "<argumentElement xmlns:sacm=\"urn:example:profile\" xmi:type=\"sacm:Proposition\" \
         sacm:weight=\"high\" xmi:id=\"c9\"><name lang=\"en\" content=\"c\"/></argumentElement>",
    );
    let output = read_with(what, &input, &options());
    let c9 = element(what, output.document(), "c9");
    assert_eq!(
        (c9.kind(), class_of(c9)),
        (ElementKind::Claim, Some(in_profile("Proposition"))),
        "{ROW}: {what}: c9 is not a Claim with the subclass Proposition"
    );
    let found = output.diagnostics();
    assert_eq!(
        tally(found),
        expected_tally(&[
            ("SACM-XMI-003", "warning", KEPT, 1),
            ("SACM-XMI-003", "warning", "SACM-XMI-003/prefix-renamed", 1),
        ]),
        "{ROW}: {what}: the diagnostics: {}",
        brief(found)
    );
    assert_subclass_kept(
        what,
        only(what, found, KEPT),
        position_of(&input, "xmi:type="),
        "sacm:Proposition",
        "Claim",
        "c9",
    );
    let renamed = only(what, found, "SACM-XMI-003/prefix-renamed");
    let arguments: BTreeMap<String, String> =
        arguments_of(renamed.arguments()).into_iter().collect();
    assert_eq!(
        (
            arguments.get("attribute").map(String::as_str),
            arguments.get("renamed_prefix").map(String::as_str),
            arguments.get("holder_id").map(String::as_str),
        ),
        (Some("sacm:weight"), Some("ns1"), Some("c9")),
        "{ROW}: {what}: the prefix-renamed of sacm:weight"
    );
    let text = assert_writes_back(what, output.document(), &options());
    assert!(
        text.contains(
            "<argumentElement xmi:type=\"ns1:Proposition\" xmi:id=\"c9\" \
             xmlns:ns1=\"urn:example:profile\" ns1:weight=\"high\">"
        ),
        "{ROW}: {what}: the tolerant write's start tag does not share ns1:\n{text}"
    );
}

// --- 9. The options --------------------------------------------------------------

fn hash_of(options: &ReadOptions) -> u64 {
    let mut hasher = DefaultHasher::new();
    options.hash(&mut hasher);
    hasher.finish()
}

fn send_sync_clone_debug<T: Send + Sync + Clone + Debug>() {}

fn copy_send_sync<T: Copy + Send + Sync>() {}

#[test]
fn lib_compat_017_read_options_hold_one_class_per_declared_name_and_compare_by_content() {
    // Replacement.
    let replaced = ReadOptions::default()
        .with_subclass(P, "B", ElementKind::Claim)
        .with_subclass(P, "B", ElementKind::ArgumentReasoning);
    assert_eq!(
        replaced.subclass(P, "B"),
        Some(ElementKind::ArgumentReasoning),
        "{ROW}: a later declaration of the same class does not replace the earlier"
    );

    // Lookups that find nothing.
    assert_eq!(
        (
            replaced.subclass(P, "C"),
            replaced.subclass("urn:other", "B"),
            ReadOptions::default().subclass(P, "B"),
        ),
        (None, None, None),
        "{ROW}: a lookup of an undeclared class finds one"
    );

    // Order independence.
    let x = ReadOptions::default()
        .with_subclass(P, "A", ElementKind::Claim)
        .with_subclass(P, "B", ElementKind::AssertedInference);
    let y = ReadOptions::default()
        .with_subclass(P, "B", ElementKind::AssertedInference)
        .with_subclass(P, "A", ElementKind::Claim);
    assert_eq!(
        x, y,
        "{ROW}: the options depend on the order of the declarations"
    );
    assert_eq!(
        hash_of(&x),
        hash_of(&y),
        "{ROW}: the hash of the options depends on the order of the declarations"
    );
    assert_ne!(
        x,
        ReadOptions::default(),
        "{ROW}: options with declarations are == the default"
    );

    // The other setters keep the declarations.
    assert_eq!(
        (
            x.clone().with_mode(Mode::Strict).subclass(P, "A"),
            x.clone().with_limits(Limits::default()).subclass(P, "A"),
        ),
        (Some(ElementKind::Claim), Some(ElementKind::Claim)),
        "{ROW}: with_mode or with_limits drops the declarations"
    );

    // Traits.
    send_sync_clone_debug::<ReadOptions>();
    copy_send_sync::<Subclass<'_>>();
}

// --- 10. Edits and compare ---------------------------------------------------------

#[test]
fn lib_compat_017_a_subclass_moves_copies_deletes_and_compares_with_its_element() {
    // (a) Move.
    let what = "(a) move";
    let mut document = g3_read().into_document();
    edit::apply(&mut document, MoveElement::new("c1", "arguments"))
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the move is refused: {error}"));
    let last = element(what, &document, "arguments")
        .children()
        .last()
        .map(|child| child.id().as_str().to_owned());
    assert_eq!(
        last.as_deref(),
        Some("c1"),
        "{ROW}: {what}: c1 is not the last child of arguments"
    );
    assert_eq!(
        class_of(element(what, &document, "c1")),
        Some(in_profile("Proposition")),
        "{ROW}: {what}: the move changes c1's subclass()"
    );
    assert_writes_back(what, &document, &options());

    // (b) Copy.
    let what = "(b) copy";
    let mut document = g3_read().into_document();
    let copy = CopyElements::new(&document, ["c1"], "arguments")
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the copy is refused: {error}"));
    edit::apply(&mut document, copy)
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the copy is refused: {error}"));
    let copied = element(what, &document, "arguments")
        .children()
        .last()
        .unwrap_or_else(|| panic!("{ROW}: {what}: arguments has no child"));
    assert_eq!(
        (
            copied.id().as_str() == "c1",
            copied.kind(),
            class_of(copied)
        ),
        (false, ElementKind::Claim, Some(in_profile("Proposition"))),
        "{ROW}: {what}: the copy is not a new Claim with the subclass Proposition"
    );

    // (c) Delete.
    let what = "(c) delete";
    let mut document = g3_read().into_document();
    edit::apply(&mut document, DeleteElement::new("i1"))
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the delete is refused: {error}"));
    assert_eq!(
        with_subclass(&document),
        ["arguments", "c1"],
        "{ROW}: {what}: the elements with a subclass() after i1 is deleted"
    );

    // (d) Moved to the roots (Q1 = (a)).
    let what = "(d) moved to the roots";
    let mut document = g3_read().into_document();
    edit::apply(&mut document, MoveElement::root("arguments"))
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the move is refused: {error}"));
    let text = assert_writes_back(what, &document, &options());
    assert!(
        text.contains("<xmi:XMI ")
            && text.contains(&format!(
                "\n  <sacm:ArgumentPackage xmi:type=\"p:Bundle\" xmi:id=\"arguments\" \
                 xmlns:p=\"{P}\">\n"
            )),
        "{ROW}: {what}: the tolerant write does not write the root's type inside xmi:XMI:\n{text}"
    );

    // (e) Compare.
    let what = "(e) compare";
    let thesis = options().with_subclass(P, "Thesis", ElementKind::Claim);
    let g3 = fixture(G3);
    let left = read_with(what, &g3, &thesis).into_document();
    let right = read_with(what, &g3.replace("p:Proposition", "p:Thesis"), &thesis).into_document();
    assert!(
        left != right,
        "{ROW}: {what}: documents whose c1 has another subclass are =="
    );
    assert_eq!(
        semantic_compare(&left, &right),
        [Difference::PreservedContent {
            id: ElementId::from("c1")
        }],
        "{ROW}: {what}: the differences"
    );
}

// --- 11. G5 and G6 ----------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one read of G5 checked against every clause of acceptance 11, in order"
)]
fn lib_compat_017_an_emf_file_with_subclass_types_reads_resolves_its_paths_and_writes_back() {
    let output = read_with("G5", &fixture(G5), &options());
    let document = output.document();

    // Ids, kinds and subclasses.
    let found: Vec<(&str, ElementKind, Option<Class>)> = document
        .elements()
        .map(|element| (element.id().as_str(), element.kind(), class_of(element)))
        .collect();
    assert_eq!(
        found,
        [
            (
                "assurance_case_package_1",
                ElementKind::AssuranceCasePackage,
                None
            ),
            (
                "argument_package_1",
                ElementKind::ArgumentPackage,
                Some(in_profile("Bundle"))
            ),
            (
                "claim_1",
                ElementKind::Claim,
                Some(in_profile("Proposition"))
            ),
            (
                "claim_2",
                ElementKind::Claim,
                Some(in_profile("Proposition"))
            ),
            (
                "asserted_inference_1",
                ElementKind::AssertedInference,
                Some(in_profile("Step"))
            ),
        ],
        "{ROW}: G5: the ids, kinds and subclasses of the elements"
    );

    // References.
    let inference = element("G5", document, "asserted_inference_1");
    assert_eq!(
        (strs(inference.sources()), strs(inference.targets())),
        (vec!["claim_2"], vec!["claim_1"]),
        "{ROW}: G5: the paths are not read as the ids of the elements they name"
    );

    // Diagnostics: 20 in all.
    let diagnostics = output.diagnostics();
    assert_eq!(
        tally(diagnostics),
        expected_tally(&[
            ("SACM-XMI-003", "warning", KEPT, 4),
            ("SACM-XMI-004", "info", "SACM-XMI-004/generated-id", 5),
            ("SACM-XMI-008", "warning", "SACM-XMI-008/other-revision", 1),
            ("SACM-XMI-010", "warning", "SACM-XMI-010/xsi-type", 4),
            ("SACM-XMI-011", "info", "SACM-XMI-011/version-on-root", 1),
            (
                "SACM-XMI-013",
                "warning",
                "SACM-XMI-013/package-role-read",
                3
            ),
            (
                "SACM-XMI-015",
                "warning",
                "SACM-XMI-015/emf-xmi-namespace-read",
                1
            ),
            ("SACM-XMI-017", "warning", "SACM-XMI-017/paths-read", 1),
        ]),
        "{ROW}: G5: the diagnostics, as (code, severity, template) and count: {}",
        brief(diagnostics)
    );
    let at = |places: &[(u32, u32)]| -> Vec<Option<SourceLocation>> {
        places
            .iter()
            .map(|&(line, column)| Some(loc(line, column)))
            .collect()
    };
    let types = [(5, 20), (7, 27), (10, 27), (13, 27)];
    for (template, places) in [
        (
            "SACM-XMI-004/generated-id",
            &[(3, 1), (5, 3), (7, 5), (10, 5), (13, 5)][..],
        ),
        ("SACM-XMI-008/other-revision", &[(3, 1)][..]),
        ("SACM-XMI-011/version-on-root", &[(3, 37)][..]),
        ("SACM-XMI-015/emf-xmi-namespace-read", &[(3, 37)][..]),
        (
            "SACM-XMI-013/package-role-read",
            &[(7, 5), (10, 5), (13, 5)][..],
        ),
        ("SACM-XMI-010/xsi-type", &types[..]),
        (KEPT, &types[..]),
        ("SACM-XMI-017/paths-read", &[(13, 45)][..]),
    ] {
        assert_eq!(
            locations(diagnostics, template),
            at(places),
            "{ROW}: G5: the locations of {template}"
        );
    }
    let kept = with_template(diagnostics, KEPT);
    for (diagnostic, (place, (written_type, class, id))) in kept.iter().zip(types.iter().zip([
        ("p:Bundle", "ArgumentPackage", "argument_package_1"),
        ("p:Proposition", "Claim", "claim_1"),
        ("p:Proposition", "Claim", "claim_2"),
        ("p:Step", "AssertedInference", "asserted_inference_1"),
    ])) {
        assert_subclass_kept(
            "G5",
            diagnostic,
            loc(place.0, place.1),
            written_type,
            class,
            id,
        );
    }
    let paths = only("G5", diagnostics, "SACM-XMI-017/paths-read");
    assert_eq!(
        (strs(paths.affected()), arguments_of(paths.arguments())),
        (
            vec!["asserted_inference_1"],
            owned(&[
                ("count", "2"),
                ("first", "//@argumentPackage.0/@argumentationElement.1")
            ]),
        ),
        "{ROW}: G5: the affected ids and arguments of SACM-XMI-017"
    );

    // Validation.
    let findings = validation::validate(document);
    assert!(
        findings.is_empty(),
        "{ROW}: G5: validate finds something: {findings:#?}"
    );

    // Write-back: G6, which reads back ==.
    let g6 = fixture(G6);
    assert_eq!(
        tolerant_write("G5", document),
        g6,
        "{ROW}: G5: the tolerant write is not G6 byte for byte"
    );
    let back = read_with("G6", &g6, &options()).into_document();
    assert!(
        back == *document,
        "{ROW}: G6: read with the declarations, it is not == the read of G5: {:?}",
        semantic_compare(document, &back)
    );
}

// --- 12. The template ------------------------------------------------------------

#[test]
fn lib_compat_017_the_template_renders_and_serializes() {
    let output = g3_read();
    let c1 = output
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.template() == KEPT && strs(diagnostic.affected()) == ["c1"])
        .unwrap_or_else(|| {
            panic!(
                "{ROW}: G3: no {KEPT} affects c1: {}",
                brief(output.diagnostics())
            )
        });
    let kinds: Vec<(&str, bool)> = c1
        .arguments()
        .iter()
        .map(|argument| (argument.name(), argument.value().as_count().is_some()))
        .collect();
    assert_eq!(
        (c1.template(), kinds),
        (KEPT, vec![("type", false), ("class", false), ("id", false)]),
        "{ROW}: the template and its Text arguments, in order"
    );
    assert_eq!(
        c1.message(),
        "the type \"p:Proposition\" of the Claim \"c1\" is not SACM 2.3, but the application \
         declared it a subclass of Claim, so the element is read as Claim and its type is kept \
         as preserved content, which a tolerant write writes back and a strict write refuses",
        "{ROW}: the message of the template"
    );

    // J3: c1's Warning as JSON, which reads back equal.
    #[cfg(feature = "serde")]
    {
        let expected = fixture(J3);
        let json = serde_json::to_string(c1)
            .unwrap_or_else(|error| panic!("{ROW}: J3: does not serialize: {error}"));
        assert_eq!(
            format!("{json}\n"),
            expected,
            "{ROW}: J3: the JSON of the Warning"
        );
        let back: Diagnostic = serde_json::from_str(&expected)
            .unwrap_or_else(|error| panic!("{ROW}: J3: does not read back: {error}"));
        assert_eq!(&back, c1, "{ROW}: J3: does not read back equal");
    }
}

// --- Added in step 2, for decisions no acceptance test above tells apart ---------

/// The XML namespace, which the prefix `xml` is bound to without a
/// declaration.
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The root ArgumentPackage `a`, binding `sacm`, `xmi`, `xsi` and `p`, with
/// the type attributes `types`.
fn root_typed(types: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:xsi=\"{XSI_NS}\" xmlns:p=\"{P}\" xmlns:q=\"{P}\" {types} xmi:id=\"a\">\
         <name lang=\"en\" content=\"a\"/></sacm:ArgumentPackage>"
    )
}

/// A root reads a subclass only when all its type attributes name that one
/// class, as a child's must agree (ADR 0044 D3; slice 0075b Q1); otherwise
/// each type is read or kept as without declarations, so no start tag is
/// written with two `xmi:type`s.
#[test]
fn lib_compat_017_a_root_reads_a_subclass_only_when_its_types_agree() {
    let declared = options().with_subclass(P, "Module", ElementKind::ArgumentPackage);

    // (a) Agreeing, under two prefixes: read, the first giving the type.
    let what = "(a) a root's agreeing types";
    let input = root_typed("xmi:type=\"p:Bundle\" xsi:type=\"q:Bundle\"");
    let output = read_with(what, &input, &declared);
    let root = element(what, output.document(), "a");
    assert_eq!(
        class_of(root),
        Some(in_profile("Bundle")),
        "{ROW}: {what}: a has not the subclass Bundle"
    );
    assert_eq!(
        tally(output.diagnostics()),
        expected_tally(&[
            ("SACM-XMI-003", "warning", KEPT, 1),
            ("SACM-XMI-010", "warning", "SACM-XMI-010/xsi-type", 1),
        ]),
        "{ROW}: {what}: the diagnostics: {}",
        brief(output.diagnostics())
    );
    assert_subclass_kept(
        what,
        only(what, output.diagnostics(), KEPT),
        position_of(&input, "xmi:type="),
        "p:Bundle",
        "ArgumentPackage",
        "a",
    );
    let text = assert_writes_back(what, output.document(), &declared);
    assert!(
        text.contains(" xmi:type=\"p:Bundle\" xmi:id=\"a\" xmlns:p=") && !text.contains("xsi:type"),
        "{ROW}: {what}: the tolerant write does not write the one type:\n{text}"
    );

    // (b) Two subclasses of the root's class that differ, and (c) a SACM
    // type beside a subclass: neither is read as a subclass.
    for (what, types, kept) in [
        (
            "(b) a root's two subclasses",
            "xmi:type=\"p:Bundle\" xsi:type=\"p:Module\"",
            &["xmi:type", "xsi:type"][..],
        ),
        (
            "(c) a root's SACM type and subclass",
            "xmi:type=\"sacm:ArgumentPackage\" xsi:type=\"p:Bundle\"",
            &["xsi:type"][..],
        ),
    ] {
        let input = root_typed(types);
        let output = read_with(what, &input, &declared);
        let root = element(what, output.document(), "a");
        assert_eq!(
            (root.kind(), class_of(root)),
            (ElementKind::ArgumentPackage, None),
            "{ROW}: {what}: a is not an ArgumentPackage without a subclass()"
        );
        let attributes: Vec<String> = with_template(output.diagnostics(), ATTRIBUTE_KEPT)
            .iter()
            .map(|diagnostic| arguments_of(diagnostic.arguments())[0].1.clone())
            .collect();
        assert_eq!(
            (attributes, with_template(output.diagnostics(), KEPT).len()),
            (kept.iter().map(|&name| name.to_owned()).collect(), 0),
            "{ROW}: {what}: the attributes kept, and the subclass-type-kept Warnings: {}",
            brief(output.diagnostics())
        );
        let text = assert_writes_back(what, output.document(), &declared);
        assert!(
            text.matches("xmi:type=").count() <= 1,
            "{ROW}: {what}: the tolerant write writes two xmi:types:\n{text}"
        );
    }
}

/// Two type attributes agree on a subclass only in the same namespace: the
/// same local name in another declared namespace disagrees.
#[test]
fn lib_compat_017_types_with_one_name_in_two_namespaces_disagree() {
    let what = "one name in two declared namespaces";
    let declared = options().with_subclass("urn:other", "Proposition", ElementKind::Claim);
    let input = doc(&child(
        "xmlns:r=\"urn:other\" xmi:type=\"p:Proposition\" xsi:type=\"r:Proposition\"",
    ));
    let output = read_with(what, &input, &declared);
    let [kept] = output.diagnostics() else {
        panic!(
            "{ROW}: {what}: not exactly one diagnostic: {}",
            brief(output.diagnostics())
        );
    };
    assert_element_kept(
        what,
        kept,
        Severity::Warning,
        loc(3, 3),
        ("ArgumentPackage", "a"),
    );
}

/// The written prefix: `xmi` bound to XMI 2.5.1 needs no binding (ADR 0035's
/// rule for a preserved attribute's prefix), so the document element's
/// start tag carries it once. A type with the prefix `xml`, whose binding
/// no start tag declares, the prefix `xmlns`, which binds no name, an empty
/// namespace, and an unprefixed type, even under a default namespace, name
/// no subclass.
#[test]
fn lib_compat_017_a_type_names_a_subclass_only_by_a_prefix_bound_to_its_namespace() {
    let root_with = |value: &str| {
        format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
             xmi:type=\"{value}\" xmi:id=\"a\"><name lang=\"en\" content=\"a\"/>\
             </sacm:ArgumentPackage>"
        )
    };

    // (a) The prefix xmi bound to XMI 2.5.1, on a root: no second binding.
    let what = "(a) the prefix xmi";
    let declared =
        ReadOptions::default().with_subclass(XMI_NS, "Bundle", ElementKind::ArgumentPackage);
    let output = read_with(what, &root_with("xmi:Bundle"), &declared);
    let root = element(what, output.document(), "a");
    assert_eq!(
        class_of(root),
        Some((XMI_NS.to_owned(), "Bundle".to_owned())),
        "{ROW}: {what}: a has not the subclass Bundle"
    );
    let text = assert_writes_back(what, output.document(), &declared);
    assert!(
        text.contains(&format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
             xmi:type=\"xmi:Bundle\" xmi:id=\"a\">\n"
        )),
        "{ROW}: {what}: the tolerant write binds xmi twice:\n{text}"
    );

    // (b) The prefix xml: the type is kept as an attribute, as without a
    // declaration.
    let what = "(b) the prefix xml";
    let declared =
        ReadOptions::default().with_subclass(XML_NS, "Bundle", ElementKind::ArgumentPackage);
    assert_reads_as_without(what, &root_with("xml:Bundle"), &declared);

    // (c) The prefix xmlns, and (d) a prefix bound to an empty namespace:
    // no binding gives either, since the read refuses both declarations as
    // malformed XML (Namespaces in XML 1.0 §3), with declarations or
    // without. So an empty declared namespace matches no type.
    let profile_and_empty = options().with_subclass("", "Proposition", ElementKind::Claim);
    for (what, attributes, template) in [
        (
            "(c) the prefix xmlns",
            "xmlns:xmlns=\"urn:example:profile\" xmi:type=\"xmlns:Proposition\"",
            "SACM-XML-001/xmlns-prefix-declared",
        ),
        (
            "(d) an empty namespace",
            "xmlns:e=\"\" xmi:type=\"e:Proposition\"",
            "SACM-XML-001/prefix-undeclared",
        ),
    ] {
        let input = doc(&child(attributes));
        let error = refusal_with(what, &input, &profile_and_empty);
        assert_eq!(
            (error.code(), error.template()),
            (Code::XML_MALFORMED, template),
            "{ROW}: {what}: the read does not refuse the declaration as malformed XML: {error}"
        );
    }

    // (e) An unprefixed type of a root under a default namespace.
    let what = "(e) an unprefixed root type";
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmlns=\"{P}\" \
         xmi:type=\"Bundle\" xmi:id=\"a\"/>"
    );
    let output = read_with(what, &input, &options());
    let root = element(what, output.document(), "a");
    assert_eq!(
        (
            class_of(root),
            with_template(output.diagnostics(), ATTRIBUTE_KEPT).len()
        ),
        (None, 1),
        "{ROW}: {what}: the type is not kept as an attribute: {}",
        brief(output.diagnostics())
    );
}

/// An element whose start tag carries `type` in both XMI namespaces, EMF's
/// and XMI 2.5.1's, reads as without declarations: the reader takes the
/// first as its type and keeps the second as written, so a subclass read
/// from it would be written back beside a second `xmi:type`, and the file
/// would not open (verifier round 1, B1).
#[test]
fn lib_compat_017_a_type_in_both_xmi_namespaces_reads_as_without_declarations() {
    let open = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" \
         xmlns:x=\"{XMI_NS}\" xmlns:p=\"urn:p\""
    );
    let child_input = format!(
        "{open} xmi:id=\"a\"><argumentElement xmi:type=\"p:Prop\" x:type=\"sacm:Claim\" \
         xmi:id=\"c1\"/></sacm:ArgumentPackage>"
    );
    let root_input = format!("{open} xmi:type=\"p:Bundle\" x:type=\"p:Bundle\" xmi:id=\"a\"/>");
    for (what, input, declared) in [
        (
            "a child typed in both XMI namespaces",
            child_input,
            ReadOptions::default().with_subclass("urn:p", "Prop", ElementKind::Claim),
        ),
        (
            "a root typed in both XMI namespaces",
            root_input,
            ReadOptions::default().with_subclass("urn:p", "Bundle", ElementKind::ArgumentPackage),
        ),
    ] {
        // (i) The read with the declaration is the read without it.
        assert_reads_as_without(what, &input, &declared);
        let output = read_with(what, &input, &declared);
        assert!(
            with_subclass(output.document()).is_empty(),
            "{ROW}: {what}: the read gives a subclass()"
        );
        // (ii) Its tolerant write opens, with the declaration, as ==.
        assert_writes_back(what, output.document(), &declared);
    }
}

/// Only `type` in both XMI namespaces turns the declarations off: an element
/// typed in XMI 2.5.1's namespace that carries another attribute in EMF's
/// still reads as its declared subclass, as a child and as a root (added
/// for a surviving hand mutation of fix round 1).
#[test]
fn lib_compat_017_a_type_in_one_xmi_namespace_beside_another_xmi_attribute_reads_as_a_subclass() {
    let open = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:e=\"{EMF_NS}\" xmlns:p=\"urn:p\""
    );
    let child_input = format!(
        "{open} xmi:id=\"a\"><argumentElement xmi:type=\"p:Prop\" e:note=\"n\" \
         xmi:id=\"c1\"/></sacm:ArgumentPackage>"
    );
    let root_input = format!("{open} xmi:type=\"p:Bundle\" e:note=\"n\" xmi:id=\"a\"/>");
    for (what, input, declared, id, class) in [
        (
            "a child typed in XMI 2.5.1's namespace beside an EMF attribute",
            child_input,
            ReadOptions::default().with_subclass("urn:p", "Prop", ElementKind::Claim),
            "c1",
            "Prop",
        ),
        (
            "a root typed in XMI 2.5.1's namespace beside an EMF attribute",
            root_input,
            ReadOptions::default().with_subclass("urn:p", "Bundle", ElementKind::ArgumentPackage),
            "a",
            "Bundle",
        ),
    ] {
        let output = read_with(what, &input, &declared);
        assert_eq!(
            class_of(element(what, output.document(), id)),
            Some(("urn:p".to_owned(), class.to_owned())),
            "{ROW}: {what}: {id} has not the subclass {class}: {}",
            brief(output.diagnostics())
        );
        assert_eq!(
            with_template(output.diagnostics(), KEPT).len(),
            1,
            "{ROW}: {what}: not one {KEPT}: {}",
            brief(output.diagnostics())
        );
        assert_writes_back(what, output.document(), &declared);
    }
}

/// A subclass's namespace text, its URI and its binding, counts against the
/// read's size limit, as a preserved attribute's does (ADR 0041 D2).
#[test]
fn lib_compat_017_a_subclass_counts_its_namespace_text_against_the_size_limit() {
    let what = "a long namespace";
    let long = format!("urn:example:profile:{}", "x".repeat(20_000));
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:p=\"{long}\" xmi:id=\"a\"><name lang=\"en\" content=\"{}\"/>\n\
         <argumentElement xmi:type=\"p:Proposition\" xmi:id=\"c1\"/></sacm:ArgumentPackage>",
        "a".repeat(2_000)
    );
    let limits = Limits::default().with_max_input_bytes(
        u64::try_from(input.len()).unwrap_or_else(|_| panic!("{ROW}: setup: the input's size")),
    );
    // Without the declaration, the element is kept whole within the limit.
    let plain = read_with(what, &input, &ReadOptions::default().with_limits(limits));
    assert!(
        plain.document().get("c1").is_none(),
        "{ROW}: {what}: without a declaration, c1 is read"
    );
    // With it, the URI and the binding (prefix + URI + 10) of the class pass
    // the limit at the type.
    let declared = ReadOptions::default()
        .with_subclass(long.as_str(), "Proposition", ElementKind::Claim)
        .with_limits(limits);
    let error = refusal_with(what, &input, &declared);
    assert_eq!(
        (
            matches!(error, ReadError::LimitExceeded { .. }),
            error.template(),
            error.location()
        ),
        (
            true,
            "SACM-SEC-003/kept-namespaces",
            Some(position_of(&input, "xmi:type=\"p:"))
        ),
        "{ROW}: {what}: the refusal: {error:?}"
    );
}
