//! Tests for the edits that create and remove cross-document references
//! (LIB-EDIT-020; ADR 0040 amendment 1, A5 and A7; XMI 2.5.1 formal/15-06-07
//! §7.10.2 "The URI reference must be of the form `URI#id_value`"; SACM 2.3
//! formal/23-05-08 §9.4 "external to Packages to the `AssuranceCasePackage`
//! that contains this `AssuranceCasePackageBinding`").
//!
//! - `Href::new` accepts exactly the values `Href::as_str()` can give, and
//!   refuses anything else with the first `HrefError` in the order of its
//!   variants.
//! - `SetReferences::with_hrefs` sets an end's `href`s beside its ids,
//!   `AddReference::href` appends one and `RemoveReference::href` removes
//!   every occurrence of one (`EditError::HrefNotHeld` when the end holds
//!   none).
//! - The four interface creates take `implements` as an `Href`, and the four
//!   binding creates take `participantPackage` `Href`s beside their ids.
//! - `LangString::with_expression_href` sets an `expression` `Href`.
//! - Bounds count distinct ids plus distinct `href`s (an upper bound, every
//!   value), and an `Href` is never checked for existence or type.
//! - The JSON forms (ADR 0040 amendment 1, A7).

use sacm::compare::semantic_compare;
use sacm::diagnostic::Code;
use sacm::edit::{
    self, AddReference, CopyElements, CreateArgumentPackage, CreateArgumentPackageBinding,
    CreateArgumentPackageInterface, CreateArtifactPackage, CreateArtifactPackageBinding,
    CreateArtifactPackageInterface, CreateAssuranceCasePackage, CreateAssuranceCasePackageBinding,
    CreateAssuranceCasePackageInterface, CreateClaim, CreateTerminologyPackage,
    CreateTerminologyPackageBinding, CreateTerminologyPackageInterface, EditError, Mutation,
    Operation, RemoveReference, SetName, SetReferences,
};
use sacm::model::{
    Document, ElementId, ElementKind, Href, HrefError, LangString, ReferenceEnd, SacmElement,
};
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-020";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The `href`s of the samples.
const H1: &str = "o.xmi#h1";
const H2: &str = "o.xmi#h2";

// --- Values ----------------------------------------------------------------------

/// `Href::new(value)`, which must accept it.
fn href(value: &str) -> Href {
    match Href::new(value) {
        Ok(href) => href,
        Err(error) => panic!("{ROW}: setup: Href::new({value:?}) refused it: {error}"),
    }
}

/// An English name.
fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// No ids.
const NO_IDS: [&str; 0] = [];

// --- The document ------------------------------------------------------------------

/// An element of `class` in the role `role`, `id`, with `attributes`, named
/// after its id, holding `body` after its name.
fn el(role: &str, class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// The AssuranceCasePackage `case`, read with no diagnostic. It holds:
///
/// - the ArgumentPackage `p`, with the Claims `c1` and `c2`, the
///   `AssertedInference` `inf` from `c2` to `c1`, the interface `e` whose
///   `implements` is the href `o.xmi#p`, and the interface `i` implementing
///   `p`;
/// - the ArgumentPackage `q`;
/// - the binding `b`, with the `participantPackage` ids `ids` and hrefs
///   `hrefs`;
/// - the ArtifactPackage `arts` and the TerminologyPackage `terms`.
fn case_with_b(ids: &[&str], hrefs: &[&str]) -> Document {
    let participants: String = ids
        .iter()
        .map(|id| format!("<participantPackage xmi:idref=\"{id}\"/>"))
        .chain(
            hrefs
                .iter()
                .map(|href| format!("<participantPackage href=\"{href}\"/>")),
        )
        .collect();
    let p = el(
        "argumentPackage",
        "ArgumentPackage",
        "p",
        "",
        &format!(
            "{}{}{}{}{}",
            el("argumentElement", "Claim", "c1", "", ""),
            el("argumentElement", "Claim", "c2", "", ""),
            el(
                "argumentElement",
                "AssertedInference",
                "inf",
                " source=\"c2\" target=\"c1\"",
                ""
            ),
            el(
                "argumentElement",
                "ArgumentPackageInterface",
                "e",
                "",
                "<implements href=\"o.xmi#p\"/>"
            ),
            el(
                "argumentElement",
                "ArgumentPackageInterface",
                "i",
                " implements=\"p\"",
                ""
            ),
        ),
    );
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>{p}{}{}{}{}\
         </sacm:AssuranceCasePackage>",
        el("argumentPackage", "ArgumentPackage", "q", "", ""),
        el(
            "argumentPackage",
            "ArgumentPackageBinding",
            "b",
            "",
            &participants
        ),
        el("artifactPackage", "ArtifactPackage", "arts", "", ""),
        el("terminologyPackage", "TerminologyPackage", "terms", "", ""),
    );
    read_clean("the case", Mode::Tolerant, &input)
}

/// [`case_with_b`] with the ids `[p]` and the hrefs `[o.xmi#h1]`.
fn the_case() -> Document {
    case_with_b(&["p"], &[H1])
}

/// Reads `input` in `mode`, which must give a document with no diagnostic.
fn read_clean(what: &str, mode: Mode, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => {
            assert_eq!(
                output.diagnostics(),
                [],
                "{ROW}: {what}: a {mode:?} read gives diagnostics"
            );
            output.into_document()
        }
        Err(error) => {
            panic!("{ROW}: {what}: a {mode:?} read refused it: {error} ({error:?})\n{input}")
        }
    }
}

/// `doc` written in `mode`, which must succeed.
fn written(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused it: {error} ({error:?})"),
    }
}

/// Writes `doc` in both modes, which must give the same bytes; reads the
/// output back in both modes, each with no diagnostic and `==`; and writes
/// what was read back, which must give the same bytes. Returns the output.
fn round_trip(what: &str, doc: &Document) -> String {
    let text = written(what, doc, Mode::Strict);
    assert_eq!(
        written(what, doc, Mode::Tolerant),
        text,
        "{ROW}: {what}: the two write modes give different bytes"
    );
    for mode in MODES {
        let back = read_clean(&format!("{what}: the output"), mode, &text);
        assert!(
            back == *doc,
            "{ROW}: {what}: the output read back in {mode:?} mode is not ==: {:?}\n{text}",
            semantic_compare(doc, &back)
        );
        assert_eq!(
            written(what, &back, Mode::Strict),
            text,
            "{ROW}: {what}: the second write is not byte-stable"
        );
    }
    text
}

/// The trimmed lines of `text`.
fn lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

/// The ids of the end `end` of `id`.
fn ids(doc: &Document, id: &str, end: ReferenceEnd) -> Vec<String> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    element
        .references(end)
        .iter()
        .map(|one| one.as_str().to_owned())
        .collect()
}

/// The `href`s of the end `end` of `id`, as strings.
fn hrefs(doc: &Document, id: &str, end: ReferenceEnd) -> Vec<String> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    element
        .hrefs(end)
        .iter()
        .map(|href| href.as_str().to_owned())
        .collect()
}

/// The ids and the `href`s of the `participantPackage` of `id`.
fn participants(doc: &Document, id: &str) -> (Vec<String>, Vec<String>) {
    (
        ids(doc, id, ReferenceEnd::ParticipantPackage),
        hrefs(doc, id, ReferenceEnd::ParticipantPackage),
    )
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
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

/// Asserts that `error` is `TooFewReferences` of `end`, `given` and
/// `required`.
fn assert_too_few(what: &str, error: &EditError, end: &str, given: usize, required: usize) {
    let EditError::TooFewReferences {
        end: found_end,
        given: found_given,
        required: found_required,
        ..
    } = error
    else {
        panic!("{ROW}: {what}: not TooFewReferences: {error:?}");
    };
    assert_eq!(
        (*found_end, *found_given, *found_required),
        (end, given, required),
        "{ROW}: {what}: TooFewReferences"
    );
    assert_eq!(
        error.code(),
        Code::MULTIPLICITY_VIOLATION,
        "{ROW}: {what}: code()"
    );
}

/// Asserts that `error` is `TooManyReferences` of `end`, `given` and
/// `allowed`.
fn assert_too_many(what: &str, error: &EditError, end: &str, given: usize, allowed: usize) {
    let EditError::TooManyReferences {
        end: found_end,
        given: found_given,
        allowed: found_allowed,
        ..
    } = error
    else {
        panic!("{ROW}: {what}: not TooManyReferences: {error:?}");
    };
    assert_eq!(
        (*found_end, *found_given, *found_allowed),
        (end, given, allowed),
        "{ROW}: {what}: TooManyReferences"
    );
    assert_eq!(
        error.code(),
        Code::MULTIPLICITY_VIOLATION,
        "{ROW}: {what}: code()"
    );
}

// =================================================================================
// 1. Href::new
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn lib_edit_020_href_new_accepts_exactly_what_the_reader_holds() {
    fn value<
        T: Clone + Copy + std::fmt::Debug + Eq + std::hash::Hash + std::error::Error + Send + Sync,
    >() {
    }
    value::<HrefError>();

    // What `as_str` can give.
    let accepted = [
        "o.xmi#c",
        "o.xmi#xpointer(id('c'))",
        "#xpointer(id('c'))",
        "sacm:Claim o.xmi#c",
        "sacm:ArgumentPackageInterface #xpointer(x)",
        "http://example.org/a/b.xmi#c_1",
        "a%20b.xmi#c",
        "b.xmi#//@argumentElement.0",
        "o.xmi#a(b",
        "#)",
        "a(1).xmi#c",
        "sacm:Term http://example.org/g.xmi#blade",
    ];
    for value in accepted {
        let built = match Href::new(value) {
            Ok(built) => built,
            Err(error) => panic!("{ROW}: Href::new({value:?}) refused it: {error:?} ({error})"),
        };
        assert_eq!(
            built.as_str(),
            value,
            "{ROW}: Href::new({value:?}).as_str()"
        );
        assert_eq!(
            Href::new(built.as_str()),
            Ok(built.clone()),
            "{ROW}: Href::new(h.as_str()) is not Ok(h) for {value:?}"
        );
    }
    // Every `Href` the reader holds is one `Href::new` gives, and the other
    // way round: the same values, read as the participants of a binding.
    let doc = case_with_b(&[], &accepted);
    let read = doc
        .get("b")
        .map(|b| b.hrefs(ReferenceEnd::ParticipantPackage).to_vec())
        .unwrap_or_default();
    assert_eq!(
        read,
        accepted.map(href),
        "{ROW}: the hrefs read are not the hrefs Href::new gives"
    );
    for held in &read {
        assert_eq!(
            Href::new(held.as_str()).as_ref(),
            Ok(held),
            "{ROW}: Href::new(h.as_str()) is not Ok(h) for the href read {held:?}"
        );
    }

    // What it refuses, with the first problem in the order of the variants.
    let refused = [
        // Character: what is no XML 1.0 Char (XML 1.0 §2.2), anywhere.
        ("o.xmi#a\u{1}", HrefError::Character),
        ("o.xmi#a\u{FFFE}", HrefError::Character),
        ("o\u{0}.xmi#a", HrefError::Character),
        ("sacm:Claim o.xmi#\u{1}", HrefError::Character),
        // Character comes before WhiteSpace.
        (" o.xmi#a\u{1}", HrefError::Character),
        // WhiteSpace: any XML white space but the one space after a type.
        (" o.xmi#c", HrefError::WhiteSpace),
        ("o.xmi#c ", HrefError::WhiteSpace),
        ("sacm:Claim  o.xmi#c", HrefError::WhiteSpace),
        ("o.xmi#a\tb", HrefError::WhiteSpace),
        ("o.xmi#a\nb", HrefError::WhiteSpace),
        ("o.xmi#a\rb", HrefError::WhiteSpace),
        ("sacm:Claim\to.xmi#c", HrefError::WhiteSpace),
        ("sacm:Claim o.xmi#c\n", HrefError::WhiteSpace),
        ("sacm:Claim o.xmi #c", HrefError::WhiteSpace),
        ("\tsacm:Claim o.xmi#c", HrefError::WhiteSpace),
        // WhiteSpace comes before Type, NoFragment and SameDocument.
        ("foo  o.xmi", HrefError::WhiteSpace),
        (" #c", HrefError::WhiteSpace),
        ("sacm:Claim  #c", HrefError::WhiteSpace),
        // Type: what stands before the one space is not `sacm:` and a
        // concrete class.
        ("foo o.xmi#c", HrefError::Type),
        ("x:Claim o.xmi#c", HrefError::Type),
        ("sacm:Assertion o.xmi#c", HrefError::Type),
        ("sacm:Nope o.xmi#c", HrefError::Type),
        ("sacm:claim o.xmi#c", HrefError::Type),
        ("Claim o.xmi#c", HrefError::Type),
        ("p:Claim o.xmi#c", HrefError::Type),
        ("sacm: o.xmi#c", HrefError::Type),
        (":Claim o.xmi#c", HrefError::Type),
        // Type comes before NoFragment, HashInFragment and SameDocument.
        ("x o.xmi", HrefError::Type),
        ("x o.xmi#a#b", HrefError::Type),
        ("foo #c", HrefError::Type),
        // NoFragment.
        ("o.xmi", HrefError::NoFragment),
        ("o.xmi#", HrefError::NoFragment),
        ("", HrefError::NoFragment),
        ("#", HrefError::NoFragment),
        ("sacm:Claim o.xmi", HrefError::NoFragment),
        ("sacm:Claim o.xmi#", HrefError::NoFragment),
        ("sacm:Claim #", HrefError::NoFragment),
        ("sacm:Claim", HrefError::NoFragment),
        // HashInFragment, which comes before SameDocument.
        ("o.xmi#a#b", HrefError::HashInFragment),
        ("sacm:Claim o.xmi#a#b", HrefError::HashInFragment),
        ("o.xmi#xpointer(a#b)", HrefError::HashInFragment),
        ("#a#b", HrefError::HashInFragment),
        // SameDocument: an id of the same document, typed or not.
        ("#c", HrefError::SameDocument),
        ("sacm:Claim #c", HrefError::SameDocument),
    ];
    for (value, expected) in refused {
        assert_eq!(
            Href::new(value),
            Err(expected),
            "{ROW}: Href::new({value:?})"
        );
    }
    // The Display texts.
    assert_eq!(
        [
            HrefError::Character,
            HrefError::WhiteSpace,
            HrefError::Type,
            HrefError::NoFragment,
            HrefError::HashInFragment,
            HrefError::SameDocument,
        ]
        .map(|error| error.to_string()),
        [
            "the href holds a character that XML 1.0 cannot represent",
            "the href holds XML white space other than the one space after its type",
            "the type before the href's URI reference is not \"sacm:\" followed by the name of a \
             concrete SACM 2.3 class",
            "the href has no \"#\" followed by a fragment",
            "the href's fragment holds a \"#\"",
            "the href names an element of the same document by its id: use the id instead",
        ],
        "{ROW}: the Display of each HrefError"
    );
}

// =================================================================================
// 2. SetReferences::with_hrefs
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn lib_edit_020_set_references_with_hrefs_replaces_both_lists() {
    use ReferenceEnd::{CitedElement, Expression, Implements, MetaClaim, ParticipantPackage};
    let (h1, h2) = (href(H1), href(H2));
    // The accessors.
    let operation = SetReferences::new("b", ParticipantPackage, ["p"]).with_hrefs([h1.clone()]);
    assert_eq!(
        (strs(operation.ids()), operation.hrefs()),
        (vec!["p"], [h1.clone()].as_slice()),
        "{ROW}: ids() and hrefs()"
    );
    assert_eq!(
        (
            SetReferences::new("b", ParticipantPackage, ["p"])
                .hrefs()
                .len(),
            SetReferences::clear("b", ParticipantPackage).hrefs().len(),
            SetReferences::new("b", ParticipantPackage, ["p"])
                .with_hrefs([h1.clone(), h1.clone()])
                .with_hrefs([h2.clone()])
                .hrefs()
        ),
        (0, 0, [h2.clone()].as_slice()),
        "{ROW}: hrefs() of new, of clear, and of with_hrefs given twice"
    );

    // On `b` with ids [p] and hrefs [o.xmi#old]: both lists are replaced.
    let mut doc = case_with_b(&["p"], &["o.xmi#old"]);
    let mutation = applied(&mut doc, operation.clone());
    assert_eq!(
        (participants(&doc, "b"), strs(mutation.changed_ids())),
        ((owned(&["p"]), owned(&[H1])), vec!["b"]),
        "{ROW}: with_hrefs([h1]) on ids [p] and hrefs [old]"
    );
    // The same set again leaves `b` unchanged.
    let before = doc.clone();
    let mutation = applied(&mut doc, operation);
    assert!(
        mutation.changed_ids().is_empty()
            && mutation.created_ids().is_empty()
            && mutation.deleted_ids().is_empty()
            && doc == before,
        "{ROW}: the same set again changes b: {mutation:?}"
    );
    // The ids alone changing, or the hrefs alone, changes `b`; the order and
    // the repeats of the hrefs are kept.
    let mutation = applied(
        &mut doc,
        SetReferences::new("b", ParticipantPackage, ["p"]).with_hrefs([
            h2.clone(),
            h1.clone(),
            h2.clone(),
        ]),
    );
    assert_eq!(
        (participants(&doc, "b"), strs(mutation.changed_ids())),
        ((owned(&["p"]), owned(&[H2, H1, H2])), vec!["b"]),
        "{ROW}: with_hrefs([h2, h1, h2]) on the same ids"
    );
    let mutation = applied(
        &mut doc,
        SetReferences::new("b", ParticipantPackage, ["q"]).with_hrefs([
            h2.clone(),
            h1.clone(),
            h2.clone(),
        ]),
    );
    assert_eq!(
        (participants(&doc, "b"), strs(mutation.changed_ids())),
        ((owned(&["q"]), owned(&[H2, H1, H2])), vec!["b"]),
        "{ROW}: other ids with the same hrefs"
    );
    // No ids and two distinct hrefs.
    applied(
        &mut doc,
        SetReferences::new("b", ParticipantPackage, NO_IDS).with_hrefs([h1.clone(), h2.clone()]),
    );
    assert_eq!(
        participants(&doc, "b"),
        (Vec::new(), owned(&[H1, H2])),
        "{ROW}: with_hrefs([h1, h2]) and no ids"
    );
    round_trip("b with two hrefs", &doc);

    // The lower bound counts distinct ids plus distinct hrefs.
    let mut doc = the_case();
    let error = refuse(
        &mut doc,
        SetReferences::new("b", ParticipantPackage, NO_IDS).with_hrefs([h1.clone(), h1.clone()]),
    );
    assert_too_few("[h1, h1] and no ids", &error, "participantPackage", 1, 2);
    let error = refuse(
        &mut doc,
        SetReferences::new("b", ParticipantPackage, NO_IDS).with_hrefs([h1.clone()]),
    );
    assert_too_few("[h1] and no ids", &error, "participantPackage", 1, 2);
    let error = refuse(
        &mut doc,
        SetReferences::new("b", ParticipantPackage, ["p", "p"]),
    );
    assert_too_few(
        "the ids [p, p] and no hrefs, which removes the href",
        &error,
        "participantPackage",
        1,
        2,
    );
    applied(
        &mut doc,
        SetReferences::new("b", ParticipantPackage, ["p", "p"])
            .with_hrefs([h1.clone(), h1.clone()]),
    );
    assert_eq!(
        participants(&doc, "b"),
        (owned(&["p", "p"]), owned(&[H1, H1])),
        "{ROW}: the ids [p, p] with [h1, h1]: two distinct values, repeats kept"
    );
    // The upper bound counts every value.
    for (what, ids, hrefs, given) in [
        ("an id and an href", vec!["p"], vec![h1.clone()], 2),
        ("two hrefs", vec![], vec![h1.clone(), h2.clone()], 2),
        ("one href twice", vec![], vec![h1.clone(), h1.clone()], 2),
    ] {
        let error = refuse(
            &mut doc,
            SetReferences::new("i", Implements, ids).with_hrefs(hrefs),
        );
        assert_too_many(what, &error, "implements", given, 1);
    }
    applied(
        &mut doc,
        SetReferences::new("i", Implements, NO_IDS).with_hrefs([h2.clone()]),
    );
    assert_eq!(
        (ids(&doc, "i", Implements), hrefs(&doc, "i", Implements)),
        (Vec::new(), owned(&[H2])),
        "{ROW}: implements set to one href"
    );

    // `clear` empties both lists, and so does a set without hrefs.
    applied(
        &mut doc,
        SetReferences::new("c1", MetaClaim, ["c2"]).with_hrefs([h1.clone(), h2.clone()]),
    );
    assert_eq!(
        (ids(&doc, "c1", MetaClaim), hrefs(&doc, "c1", MetaClaim)),
        (owned(&["c2"]), owned(&[H1, H2])),
        "{ROW}: setup: the metaClaim of c1"
    );
    let mutation = applied(&mut doc, SetReferences::clear("c1", MetaClaim));
    assert_eq!(
        (
            ids(&doc, "c1", MetaClaim),
            hrefs(&doc, "c1", MetaClaim),
            strs(mutation.changed_ids())
        ),
        (Vec::new(), Vec::new(), vec!["c1"]),
        "{ROW}: clear does not empty both lists"
    );
    applied(
        &mut doc,
        SetReferences::new("c1", CitedElement, NO_IDS).with_hrefs([h1.clone()]),
    );
    let mutation = applied(&mut doc, SetReferences::clear("c1", CitedElement));
    assert_eq!(
        (
            hrefs(&doc, "c1", CitedElement).len(),
            strs(mutation.changed_ids())
        ),
        (0, vec!["c1"]),
        "{ROW}: clear of an end that holds only an href"
    );
    applied(
        &mut doc,
        SetReferences::new("c1", MetaClaim, NO_IDS).with_hrefs([h1.clone()]),
    );
    applied(&mut doc, SetReferences::new("c1", MetaClaim, ["c2"]));
    assert_eq!(
        (ids(&doc, "c1", MetaClaim), hrefs(&doc, "c1", MetaClaim)),
        (owned(&["c2"]), Vec::new()),
        "{ROW}: a set without hrefs keeps the hrefs"
    );

    // The checks before the bounds are unchanged; an href is never checked.
    let error = refuse(
        &mut doc,
        SetReferences::new("c1", Expression, NO_IDS).with_hrefs([h1.clone()]),
    );
    assert!(
        matches!(
            &error,
            EditError::DerivedEnd { element, kind: ElementKind::Claim, end: "expression", .. }
                if element.as_str() == "c1"
        ),
        "{ROW}: hrefs on the derived expression end: {error:?}"
    );
    let error = refuse(
        &mut doc,
        SetReferences::new("nope", MetaClaim, NO_IDS).with_hrefs([h1.clone()]),
    );
    assert!(
        matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nope"),
        "{ROW}: hrefs on an element the document does not hold: {error:?}"
    );
    let error = refuse(
        &mut doc,
        SetReferences::new("c1", ParticipantPackage, NO_IDS).with_hrefs([h1.clone(), h2.clone()]),
    );
    assert!(
        matches!(&error, EditError::FeatureNotApplicable { element, .. } if element.as_str() == "c1"),
        "{ROW}: hrefs on an end a Claim does not have: {error:?}"
    );
    let error = refuse(
        &mut doc,
        SetReferences::new("c1", MetaClaim, ["nope"]).with_hrefs([h1]),
    );
    assert!(
        matches!(&error, EditError::ReferenceNotFound { end: "metaClaim", id, .. } if id.as_str() == "nope"),
        "{ROW}: an id that names nothing beside an href: {error:?}"
    );
}

// =================================================================================
// 3. AddReference::href
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn lib_edit_020_add_reference_href_appends_and_checks_bounds() {
    use ReferenceEnd::{
        CitedElement, Expression, Implements, MetaClaim, ParticipantPackage, Target,
    };
    let (h1, h2) = (href(H1), href(H2));
    // The accessors: exactly one of an id and an href.
    let by_href = AddReference::href("b", ParticipantPackage, h2.clone());
    let by_id = AddReference::new("b", ParticipantPackage, "q");
    assert_eq!(
        (
            by_href.element().as_str(),
            by_href.end(),
            by_href.id(),
            by_href.cross_document(),
            by_id.id().map(ElementId::as_str),
            by_id.cross_document()
        ),
        ("b", ParticipantPackage, None, Some(&h2), Some("q"), None),
        "{ROW}: element(), end(), id() and cross_document()"
    );

    // Appending h2 to ids [p] and hrefs [h1].
    let mut doc = the_case();
    let mutation = applied(&mut doc, by_href);
    assert_eq!(
        (
            participants(&doc, "b"),
            strs(mutation.changed_ids()),
            mutation.created_ids().len(),
            mutation.deleted_ids().len()
        ),
        ((owned(&["p"]), owned(&[H1, H2])), vec!["b"], 0, 0),
        "{ROW}: AddReference::href of h2"
    );
    // An id is still appended to the ids, and leaves the hrefs alone.
    applied(&mut doc, by_id);
    assert_eq!(
        participants(&doc, "b"),
        (owned(&["p", "q"]), owned(&[H1, H2])),
        "{ROW}: AddReference::new of q"
    );
    round_trip("b with two ids and two hrefs", &doc);

    // The upper bound counts every value.
    let mut doc = the_case();
    let error = refuse(&mut doc, AddReference::href("e", Implements, h2.clone()));
    assert_too_many(
        "a second implements href on an interface",
        &error,
        "implements",
        2,
        1,
    );
    let error = refuse(&mut doc, AddReference::href("i", Implements, h2.clone()));
    assert_too_many(
        "an implements href on an interface that holds an id",
        &error,
        "implements",
        2,
        1,
    );
    let error = refuse(&mut doc, AddReference::href("inf", Target, h1.clone()));
    assert_too_many(
        "an href on a target that holds an id",
        &error,
        "target",
        2,
        1,
    );

    // citedElement of a Claim: appended, and written in element form.
    let mutation = applied(&mut doc, AddReference::href("c1", CitedElement, h1.clone()));
    assert_eq!(
        (
            ids(&doc, "c1", CitedElement),
            hrefs(&doc, "c1", CitedElement),
            strs(mutation.changed_ids())
        ),
        (Vec::new(), owned(&[H1]), vec!["c1"]),
        "{ROW}: AddReference::href on the citedElement of c1"
    );
    let text = round_trip("c1 with a citedElement href", &doc);
    assert!(
        lines(&text).contains(&format!("<citedElement href=\"{H1}\" />").as_str())
            && !text.contains(" citedElement=\""),
        "{ROW}: the citedElement is not written in element form\n{text}"
    );
    let error = refuse(&mut doc, AddReference::href("c1", CitedElement, h2.clone()));
    assert_too_many("a second citedElement href", &error, "citedElement", 2, 1);
    // An unbounded end keeps the order given; ids stay attributes' tokens no
    // more: the end is written in element form, the ids first.
    applied(&mut doc, AddReference::href("c2", MetaClaim, h2.clone()));
    applied(&mut doc, AddReference::new("c2", MetaClaim, "c1"));
    applied(&mut doc, AddReference::href("c2", MetaClaim, h1.clone()));
    assert_eq!(
        (ids(&doc, "c2", MetaClaim), hrefs(&doc, "c2", MetaClaim)),
        (owned(&["c1"]), owned(&[H2, H1])),
        "{ROW}: two hrefs and an id appended to the metaClaim of c2"
    );
    let text = round_trip("c2 with metaClaim hrefs", &doc);
    let found = lines(&text);
    let Some(first) = found
        .iter()
        .position(|line| *line == "<metaClaim xmi:idref=\"c1\" />")
    else {
        panic!("{ROW}: the metaClaim id is not written as an element\n{text}");
    };
    assert_eq!(
        &found[first + 1..first + 3],
        [
            format!("<metaClaim href=\"{H2}\" />"),
            format!("<metaClaim href=\"{H1}\" />")
        ],
        "{ROW}: the metaClaim hrefs are not written after the id, in order\n{text}"
    );

    // The checks before the bounds; the href itself is never checked.
    let error = refuse(&mut doc, AddReference::href("nope", MetaClaim, h1.clone()));
    assert!(
        matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nope"),
        "{ROW}: an href on an element the document does not hold: {error:?}"
    );
    let error = refuse(&mut doc, AddReference::href("c1", Expression, h1.clone()));
    assert!(
        matches!(
            &error,
            EditError::DerivedEnd {
                kind: ElementKind::Claim,
                end: "expression",
                ..
            }
        ),
        "{ROW}: an href on the derived expression end: {error:?}"
    );
    let error = refuse(&mut doc, AddReference::href("c1", ParticipantPackage, h1));
    assert!(
        matches!(&error, EditError::FeatureNotApplicable { element, .. } if element.as_str() == "c1"),
        "{ROW}: an href on an end a Claim does not have: {error:?}"
    );
}

// =================================================================================
// 4. RemoveReference::href
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn lib_edit_020_remove_reference_href_removes_every_occurrence() {
    use ReferenceEnd::{CitedElement, Expression, MetaClaim, ParticipantPackage};
    let (h1, h2) = (href(H1), href(H2));
    let absent = href("o.xmi#x");
    // The accessors.
    let by_href = RemoveReference::href("b", ParticipantPackage, h1.clone());
    let by_id = RemoveReference::new("b", ParticipantPackage, "p");
    assert_eq!(
        (
            by_href.element().as_str(),
            by_href.end(),
            by_href.id(),
            by_href.cross_document(),
            by_id.id().map(ElementId::as_str),
            by_id.cross_document()
        ),
        ("b", ParticipantPackage, None, Some(&h1), Some("p"), None),
        "{ROW}: element(), end(), id() and cross_document()"
    );

    // Removing h1 from [h1, h1, h2] leaves [h2]; the ids stay.
    let mut doc = case_with_b(&["p"], &[H1, H1, H2]);
    let mutation = applied(&mut doc, by_href);
    assert_eq!(
        (
            participants(&doc, "b"),
            strs(mutation.changed_ids()),
            mutation.deleted_ids().len()
        ),
        ((owned(&["p"]), owned(&[H2])), vec!["b"], 0),
        "{ROW}: RemoveReference::href of h1 from [h1, h1, h2]"
    );
    round_trip("b after the removal", &doc);
    // Every occurrence, wherever it stands.
    let mut doc = case_with_b(&["p", "q"], &[H1, H2, H1, H2, H1]);
    applied(
        &mut doc,
        RemoveReference::href("b", ParticipantPackage, h1.clone()),
    );
    assert_eq!(
        participants(&doc, "b"),
        (owned(&["p", "q"]), owned(&[H2, H2])),
        "{ROW}: RemoveReference::href of h1 from [h1, h2, h1, h2, h1]"
    );

    // An href the end does not hold.
    let mut doc = the_case();
    let error = refuse(
        &mut doc,
        RemoveReference::href("b", ParticipantPackage, absent.clone()),
    );
    let EditError::HrefNotHeld {
        end, href: named, ..
    } = &error
    else {
        panic!("{ROW}: not HrefNotHeld: {error:?}");
    };
    assert_eq!(
        (*end, named, error.code(), error.code().as_str()),
        (
            "participantPackage",
            &absent,
            Code::CMD_TARGET_NOT_FOUND,
            "SACM-CMD-001"
        ),
        "{ROW}: HrefNotHeld"
    );
    assert_eq!(
        error.to_string(),
        "the participantPackage holds no href \"o.xmi#x\" to remove",
        "{ROW}: the Display of HrefNotHeld"
    );
    // Hrefs are compared as strings: a typed href is not the untyped one,
    // and an id is no href.
    let mut typed = case_with_b(&["p", "q"], &["sacm:ArgumentPackage o.xmi#h1"]);
    let error = refuse(
        &mut typed,
        RemoveReference::href("b", ParticipantPackage, h1.clone()),
    );
    assert!(
        matches!(&error, EditError::HrefNotHeld { end: "participantPackage", href, .. } if *href == h1),
        "{ROW}: the untyped href removed from an end that holds the typed one: {error:?}"
    );
    applied(
        &mut typed,
        RemoveReference::href(
            "b",
            ParticipantPackage,
            href("sacm:ArgumentPackage o.xmi#h1"),
        ),
    );
    assert_eq!(
        participants(&typed, "b"),
        (owned(&["p", "q"]), Vec::new()),
        "{ROW}: the typed href removed"
    );

    // The bounds are checked after the removal, and after HrefNotHeld.
    let mut doc = the_case();
    let error = refuse(
        &mut doc,
        RemoveReference::href("b", ParticipantPackage, h1.clone()),
    );
    assert_too_few(
        "removing h1 from ids [p] and hrefs [h1]",
        &error,
        "participantPackage",
        1,
        2,
    );
    let mut repeated = case_with_b(&["p"], &[H1, H1]);
    let error = refuse(
        &mut repeated,
        RemoveReference::href("b", ParticipantPackage, h1.clone()),
    );
    assert_too_few(
        "removing h1 from ids [p] and hrefs [h1, h1]",
        &error,
        "participantPackage",
        1,
        2,
    );
    let mut below = case_with_b(&["p"], &[]);
    let error = refuse(
        &mut below,
        RemoveReference::href("b", ParticipantPackage, absent.clone()),
    );
    assert!(
        matches!(
            &error,
            EditError::HrefNotHeld {
                end: "participantPackage",
                ..
            }
        ),
        "{ROW}: an href not held, on a binding already below its bound: {error:?}"
    );
    // Removing an id still leaves the hrefs alone, and counts them.
    let mut doc = case_with_b(&["p"], &[H1, H2]);
    applied(&mut doc, RemoveReference::new("b", ParticipantPackage, "p"));
    assert_eq!(
        participants(&doc, "b"),
        (Vec::new(), owned(&[H1, H2])),
        "{ROW}: RemoveReference::new of p"
    );

    // An end with no lower bound is left empty, and written without it.
    applied(&mut doc, AddReference::href("c1", CitedElement, h2.clone()));
    let mutation = applied(
        &mut doc,
        RemoveReference::href("c1", CitedElement, h2.clone()),
    );
    assert_eq!(
        (
            hrefs(&doc, "c1", CitedElement).len(),
            strs(mutation.changed_ids())
        ),
        (0, vec!["c1"]),
        "{ROW}: the citedElement href of c1 removed"
    );
    let text = round_trip("c1 after the removal", &doc);
    assert!(
        !text.contains("<citedElement"),
        "{ROW}: the removed href is still written\n{text}"
    );

    // The checks before HrefNotHeld: the element, the derived end, the
    // feature.
    let error = refuse(
        &mut doc,
        RemoveReference::href("nope", MetaClaim, absent.clone()),
    );
    assert!(
        matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nope"),
        "{ROW}: an href removed from an element the document does not hold: {error:?}"
    );
    let error = refuse(
        &mut doc,
        RemoveReference::href("c1", Expression, absent.clone()),
    );
    assert!(
        matches!(
            &error,
            EditError::DerivedEnd {
                kind: ElementKind::Claim,
                end: "expression",
                ..
            }
        ),
        "{ROW}: an href removed from the derived expression end: {error:?}"
    );
    let error = refuse(
        &mut doc,
        RemoveReference::href("c1", ParticipantPackage, absent.clone()),
    );
    assert!(
        matches!(&error, EditError::FeatureNotApplicable { element, .. } if element.as_str() == "c1"),
        "{ROW}: an href removed from an end a Claim does not have: {error:?}"
    );
    let error = refuse(&mut doc, RemoveReference::href("c1", MetaClaim, absent));
    assert!(
        matches!(
            &error,
            EditError::HrefNotHeld {
                end: "metaClaim",
                ..
            }
        ),
        "{ROW}: an href removed from an empty end: {error:?}"
    );
}

// =================================================================================
// 5. The interface and binding creates
// =================================================================================

/// Checks the create operation `$operation` of the interface kind `$kind`.
macro_rules! check_interface_create {
    ($operation:ident, $kind:ident, $package:literal) => {{
        let what = stringify!($operation);
        let h = href("remote.sacm.xmi#package");
        // The accessors: exactly one of an id and an href.
        let root = $operation::root_href(en("Root"), h.clone()).with_id("new_root");
        let child = $operation::new_href("case", en("Child"), h.clone()).with_id("new_child");
        let by_id = $operation::new("case", en("By id"), $package);
        assert_eq!(
            (
                root.parent(),
                root.implements(),
                root.implements_href(),
                child.parent().map(ElementId::as_str),
                child.implements(),
                child.implements_href(),
                by_id.implements().map(ElementId::as_str),
                by_id.implements_href(),
                $operation::root(en("By id"), $package)
                    .implements()
                    .map(ElementId::as_str)
            ),
            (
                None,
                None,
                Some(&h),
                Some("case"),
                None,
                Some(&h),
                Some($package),
                None,
                Some($package)
            ),
            "{ROW}: {what}: parent(), implements() and implements_href()"
        );
        let mut doc = the_case();
        let roots_before = doc.roots().count();
        let mutation = applied(&mut doc, root);
        assert_eq!(
            (strs(mutation.created_ids()), doc.roots().count()),
            (vec!["new_root"], roots_before + 1),
            "{ROW}: {what}: root_href creates a root"
        );
        let mutation = applied(&mut doc, child);
        assert_eq!(
            strs(mutation.created_ids()),
            ["new_child"],
            "{ROW}: {what}: new_href creates a child"
        );
        for (id, parent) in [("new_root", None), ("new_child", Some("case"))] {
            let Some(created) = doc.get(id) else {
                panic!("{ROW}: {what}: {id} is not created");
            };
            assert_eq!(
                (
                    created.kind(),
                    created.parent().map(|parent| parent.id().as_str()),
                    created.implements(),
                    created.references(ReferenceEnd::Implements).len(),
                    created.hrefs(ReferenceEnd::Implements)
                ),
                (
                    ElementKind::$kind,
                    parent,
                    None,
                    0,
                    [h.clone()].as_slice()
                ),
                "{ROW}: {what}: {id}"
            );
        }
        round_trip(what, &doc);
        // The checks of ADR 0019 are unchanged.
        let error = refuse(
            &mut doc,
            $operation::new_href("nope", en("Child"), h.clone()),
        );
        assert!(
            matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nope"),
            "{ROW}: {what}: new_href under a parent the document does not hold: {error:?}"
        );
        let error = refuse(
            &mut doc,
            $operation::root_href(en("Root"), h.clone()).with_id("new_root"),
        );
        assert!(
            matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "new_root"),
            "{ROW}: {what}: root_href with an id the document holds: {error:?}"
        );
        // Then the name's expression: an id that names nothing is refused
        // even though the href is not checked.
        let error = refuse(
            &mut doc,
            $operation::root_href(en("Root").with_expression("nope"), h),
        );
        assert!(
            matches!(&error, EditError::ReferenceNotFound { end: "expression", id, .. } if id.as_str() == "nope"),
            "{ROW}: {what}: root_href with a name whose expression names nothing: {error:?}"
        );
    }};
}

/// Checks the create operation `$operation` of the binding kind `$kind`,
/// whose participants are packages such as `$package`.
macro_rules! check_binding_create {
    ($operation:ident, $kind:ident, $package:literal) => {{
        let what = stringify!($operation);
        let (h, other) = (href("remote.sacm.xmi#api"), href("other.sacm.xmi#api"));
        // The accessors.
        let operation = $operation::new("case", en("Binding"), [$package])
            .with_participant_package_hrefs([h.clone()])
            .with_id("new_binding");
        assert_eq!(
            (
                strs(operation.participant_packages()),
                operation.participant_package_hrefs(),
                $operation::new("case", en("Binding"), [$package])
                    .participant_package_hrefs()
                    .len()
            ),
            (vec![$package], [h.clone()].as_slice(), 0),
            "{ROW}: {what}: participant_packages() and participant_package_hrefs()"
        );
        // One id and one href: two distinct participants.
        let mut doc = the_case();
        let mutation = applied(&mut doc, operation);
        assert_eq!(
            (
                strs(mutation.created_ids()),
                doc.get("new_binding").map(|created| created.kind()),
                participants(&doc, "new_binding")
            ),
            (
                vec!["new_binding"],
                Some(ElementKind::$kind),
                (owned(&[$package]), owned(&[h.as_str()]))
            ),
            "{ROW}: {what}: a binding of one id and one href"
        );
        // The id alone is one participant.
        let error = refuse(
            &mut doc,
            $operation::new("case", en("Binding"), [$package]),
        );
        assert_too_few(
            &format!("{what}: one id alone"),
            &error,
            "participantPackage",
            1,
            2,
        );
        // Distinct hrefs count, and repeats do not; the order is kept.
        let error = refuse(
            &mut doc,
            $operation::root(en("Binding"), NO_IDS)
                .with_participant_package_hrefs([h.clone(), h.clone()]),
        );
        assert_too_few(
            &format!("{what}: one href twice"),
            &error,
            "participantPackage",
            1,
            2,
        );
        let error = refuse(
            &mut doc,
            $operation::root(en("Binding"), NO_IDS).with_participant_package_hrefs([h.clone()]),
        );
        assert_too_few(
            &format!("{what}: one href alone"),
            &error,
            "participantPackage",
            1,
            2,
        );
        applied(
            &mut doc,
            $operation::root(en("Binding"), NO_IDS)
                .with_participant_package_hrefs([other.clone(), h.clone(), other.clone()])
                .with_id("new_root_binding"),
        );
        assert_eq!(
            (
                participants(&doc, "new_root_binding"),
                doc.get("new_root_binding")
                    .is_some_and(|created| created.parent().is_none())
            ),
            (
                (
                    Vec::new(),
                    owned(&[other.as_str(), h.as_str(), other.as_str()])
                ),
                true
            ),
            "{ROW}: {what}: a root binding of hrefs alone"
        );
        round_trip(what, &doc);
        // The bounds come before the id checks, which are as before.
        let error = refuse(&mut doc, $operation::new("case", en("Binding"), ["nope"]));
        assert_too_few(
            &format!("{what}: one id that names nothing, alone"),
            &error,
            "participantPackage",
            1,
            2,
        );
        let error = refuse(
            &mut doc,
            $operation::new("case", en("Binding"), ["nope"]).with_participant_package_hrefs([h]),
        );
        assert!(
            matches!(
                &error,
                EditError::ReferenceNotFound { end: "participantPackage", id, .. }
                    if id.as_str() == "nope"
            ),
            "{ROW}: {what}: an id that names nothing, beside an href: {error:?}"
        );
    }};
}

#[test]
fn lib_edit_020_interface_and_binding_creates_take_hrefs() {
    check_interface_create!(
        CreateAssuranceCasePackageInterface,
        AssuranceCasePackageInterface,
        "case"
    );
    check_interface_create!(
        CreateArgumentPackageInterface,
        ArgumentPackageInterface,
        "p"
    );
    check_interface_create!(
        CreateArtifactPackageInterface,
        ArtifactPackageInterface,
        "arts"
    );
    check_interface_create!(
        CreateTerminologyPackageInterface,
        TerminologyPackageInterface,
        "terms"
    );
    check_binding_create!(
        CreateAssuranceCasePackageBinding,
        AssuranceCasePackageBinding,
        "case"
    );
    check_binding_create!(CreateArgumentPackageBinding, ArgumentPackageBinding, "p");
    check_binding_create!(CreateArtifactPackageBinding, ArtifactPackageBinding, "arts");
    check_binding_create!(
        CreateTerminologyPackageBinding,
        TerminologyPackageBinding,
        "terms"
    );
}

// =================================================================================
// 6. An expression href
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn lib_edit_020_an_expression_href_is_never_checked() {
    let h = href("terms.sacm.xmi#blade");
    // The value: with_expression_href keeps lang and content, and replaces
    // any earlier expression; with_expression replaces it with the id.
    let name = LangString::new("x")
        .with_lang("en")
        .with_expression_href(h.clone());
    assert_eq!(
        (
            name.expression_href(),
            name.expression(),
            name.is_expression_lang_string(),
            name.lang(),
            name.content()
        ),
        (Some(&h), None, true, Some("en"), Some("x")),
        "{ROW}: LangString::with_expression_href"
    );
    let by_id = name.clone().with_expression("t");
    assert_eq!(
        (
            by_id.expression().map(ElementId::as_str),
            by_id.expression_href(),
            by_id.is_expression_lang_string(),
            by_id.lang(),
            by_id.content()
        ),
        (Some("t"), None, true, Some("en"), Some("x")),
        "{ROW}: with_expression after with_expression_href"
    );
    let back = by_id.clone().with_expression_href(h.clone());
    assert_eq!(
        (back.expression(), back.expression_href(), back == name),
        (None, Some(&h), true),
        "{ROW}: with_expression_href after with_expression"
    );
    let other = name
        .clone()
        .with_expression_href(href("terms.sacm.xmi#hub"));
    assert_eq!(
        (
            other.expression_href().map(Href::as_str),
            other == name,
            by_id == name,
            name == name.clone()
        ),
        (Some("terms.sacm.xmi#hub"), false, false, true),
        "{ROW}: a second with_expression_href, and =="
    );
    let plain = name.clone().without_expression();
    assert_eq!(
        (
            plain.expression_href(),
            plain.is_expression_lang_string(),
            plain == LangString::new("x").with_lang("en")
        ),
        (None, false, true),
        "{ROW}: without_expression after with_expression_href"
    );

    // CreateClaim and SetName accept it, whatever it names.
    let mut doc = the_case();
    let mutation = applied(&mut doc, CreateClaim::new("p", name.clone()).with_id("c9"));
    assert_eq!(
        (
            strs(mutation.created_ids()),
            doc.get("c9").and_then(SacmElement::name),
            doc.get("c9").map(|c9| c9.expressions().len())
        ),
        (vec!["c9"], Some(&name), Some(0)),
        "{ROW}: CreateClaim with an expression href"
    );
    let mutation = applied(&mut doc, SetName::new("c1", name.clone()));
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            doc.get("c1").and_then(SacmElement::name)
        ),
        (vec!["c1"], Some(&name)),
        "{ROW}: SetName with an expression href"
    );
    // The control: an expression id that names nothing is refused.
    let error = refuse(
        &mut doc,
        SetName::new("c2", LangString::new("x").with_expression("nope")),
    );
    assert!(
        matches!(&error, EditError::ReferenceNotFound { end: "expression", id, .. } if id.as_str() == "nope"),
        "{ROW}: setup: an expression id that names nothing is accepted: {error:?}"
    );
    // Another href replaces it.
    let mutation = applied(&mut doc, SetName::new("c1", other.clone()));
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            doc.get("c1").and_then(SacmElement::name)
        ),
        (vec!["c1"], Some(&other)),
        "{ROW}: SetName with another expression href"
    );
    applied(&mut doc, SetName::new("c1", name.clone()));

    // A copy keeps it.
    let copy = CopyElements::new(&doc, ["c9"], "q")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy of c9: {error}"));
    let mutation = applied(&mut doc, copy);
    let Some(copied) = mutation.created_ids().first().cloned() else {
        panic!("{ROW}: the copy created nothing");
    };
    assert_eq!(
        (
            doc.get(&copied).and_then(SacmElement::name),
            doc.get(&copied)
                .and_then(SacmElement::parent)
                .map(|parent| parent.id().as_str())
        ),
        (Some(&name), Some("q")),
        "{ROW}: the copy does not keep the expression href"
    );
    // Written as an expression element, and read back.
    let text = round_trip("names with an expression href", &doc);
    assert_eq!(
        lines(&text)
            .iter()
            .filter(|line| **line == "<expression href=\"terms.sacm.xmi#blade\" />")
            .count(),
        3,
        "{ROW}: not one expression element for each of c1, c9 and the copy\n{text}"
    );
}

// =================================================================================
// 7. serde
// =================================================================================

/// The directory of the golden JSON files.
#[cfg(feature = "serde")]
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

#[cfg(feature = "serde")]
fn to_json<T: serde::Serialize + std::fmt::Debug>(value: &T) -> String {
    match serde_json::to_string(value) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {value:?} does not serialize: {error}"),
    }
}

#[cfg(feature = "serde")]
fn from_json<T: serde::de::DeserializeOwned>(text: &str) -> T {
    match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => panic!("{ROW}: {text} does not read: {error}"),
    }
}

/// Why `text` does not read as a `T`, which it must not.
#[cfg(feature = "serde")]
fn refused<T: serde::de::DeserializeOwned + std::fmt::Debug>(text: &str) -> String {
    match serde_json::from_str::<T>(text) {
        Ok(value) => panic!("{ROW}: {text} reads, as {value:?}"),
        Err(error) => error.to_string(),
    }
}

/// The items of the golden file `name`, a JSON array.
#[cfg(feature = "serde")]
fn golden_items(name: &str) -> Vec<serde_json::Value> {
    let path = std::path::Path::new(GOLDEN).join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    from_json(&text)
}

#[cfg(feature = "serde")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the samples of one acceptance item read best as one list"
)]
fn lib_edit_020_serde_reads_and_writes_hrefs() {
    {
        let remote = href("braking.sacm.xmi#braking_api");
        let participant = ReferenceEnd::ParticipantPackage;
        // The samples of `operations.json`, each with its compact JSON, keys
        // in this order.
        let samples: [(Operation, &str); 6] = [
            (
                SetReferences::new("binding", participant, ["vehicle_api"])
                    .with_hrefs([remote.clone()])
                    .into(),
                r#"{"type":"SetReferences","element":"binding","end":"participantPackage","ids":["vehicle_api"],"hrefs":["braking.sacm.xmi#braking_api"]}"#,
            ),
            (
                AddReference::href("binding", participant, remote.clone()).into(),
                r#"{"type":"AddReference","element":"binding","end":"participantPackage","href":"braking.sacm.xmi#braking_api"}"#,
            ),
            (
                RemoveReference::href("binding", participant, remote.clone()).into(),
                r#"{"type":"RemoveReference","element":"binding","end":"participantPackage","href":"braking.sacm.xmi#braking_api"}"#,
            ),
            (
                CreateArgumentPackageInterface::root_href(
                    en("Braking interface"),
                    href("braking.sacm.xmi#braking"),
                )
                .with_id("braking_api")
                .into(),
                r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":"braking_api","name":{"lang":"en","content":"Braking interface"},"implements_href":"braking.sacm.xmi#braking"}"#,
            ),
            (
                CreateArgumentPackageBinding::root(
                    en("Braking supports the vehicle"),
                    ["vehicle_api"],
                )
                .with_participant_package_hrefs([remote.clone()])
                .with_id("binding")
                .into(),
                r#"{"type":"CreateArgumentPackageBinding","parent":null,"id":"binding","name":{"lang":"en","content":"Braking supports the vehicle"},"participant_packages":["vehicle_api"],"participant_package_hrefs":["braking.sacm.xmi#braking_api"]}"#,
            ),
            (
                CreateClaim::new(
                    "arguments",
                    LangString::new("x").with_expression_href(href("terms.sacm.xmi#blade")),
                )
                .into(),
                r#"{"type":"CreateClaim","parent":"arguments","id":null,"name":{"type":"ExpressionLangString","lang":null,"content":"x","expression":{"href":"terms.sacm.xmi#blade"}}}"#,
            ),
        ];
        let operations = golden_items("operations.json");
        for (operation, json) in &samples {
            assert_eq!(
                &to_json(operation),
                json,
                "{ROW}: the JSON of {operation:?}"
            );
            assert_eq!(
                &from_json::<Operation>(json),
                operation,
                "{ROW}: {json} does not read back"
            );
            assert!(
                operations.contains(&from_json::<serde_json::Value>(json)),
                "{ROW}: operations.json does not hold {json}"
            );
        }
        // An empty list of hrefs is not written, and may be left out or
        // given; the id forms are as before.
        for (operation, json, with_empty) in [
            (
                Operation::from(SetReferences::new("i1", ReferenceEnd::Source, ["c2", "c3"])),
                r#"{"type":"SetReferences","element":"i1","end":"source","ids":["c2","c3"]}"#,
                Some(
                    r#"{"type":"SetReferences","element":"i1","end":"source","ids":["c2","c3"],"hrefs":[]}"#,
                ),
            ),
            (
                CreateArgumentPackageBinding::root(en("B"), ["a", "b"])
                    .with_id("binding")
                    .into(),
                r#"{"type":"CreateArgumentPackageBinding","parent":null,"id":"binding","name":{"lang":"en","content":"B"},"participant_packages":["a","b"]}"#,
                Some(
                    r#"{"type":"CreateArgumentPackageBinding","parent":null,"id":"binding","name":{"lang":"en","content":"B"},"participant_packages":["a","b"],"participant_package_hrefs":[]}"#,
                ),
            ),
            (
                AddReference::new("g1", ReferenceEnd::GroupMember, "c3").into(),
                r#"{"type":"AddReference","element":"g1","end":"argumentElement","id":"c3"}"#,
                None,
            ),
            (
                RemoveReference::new("i1", ReferenceEnd::Source, "c3").into(),
                r#"{"type":"RemoveReference","element":"i1","end":"source","id":"c3"}"#,
                None,
            ),
            (
                CreateArgumentPackageInterface::root(en("I"), "braking")
                    .with_id("braking_api")
                    .into(),
                r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":"braking_api","name":{"lang":"en","content":"I"},"implements":"braking"}"#,
                None,
            ),
            (
                SetReferences::new("b", participant, NO_IDS)
                    .with_hrefs([remote.clone(), remote.clone()])
                    .into(),
                r#"{"type":"SetReferences","element":"b","end":"participantPackage","ids":[],"hrefs":["braking.sacm.xmi#braking_api","braking.sacm.xmi#braking_api"]}"#,
                None,
            ),
        ] {
            assert_eq!(
                to_json(&operation),
                json,
                "{ROW}: the JSON of {operation:?}"
            );
            assert_eq!(
                from_json::<Operation>(json),
                operation,
                "{ROW}: {json} does not read back"
            );
            if let Some(with_empty) = with_empty {
                assert_eq!(
                    from_json::<Operation>(with_empty),
                    operation,
                    "{ROW}: {with_empty} does not read as the operation without hrefs"
                );
            }
        }

        // What is refused: both keys, neither, and a string that is no Href.
        for json in [
            r#"{"type":"AddReference","element":"b","end":"participantPackage","id":"p","href":"o.xmi#p"}"#,
            r#"{"type":"AddReference","element":"b","end":"participantPackage"}"#,
            r#"{"type":"RemoveReference","element":"b","end":"participantPackage","id":"p","href":"o.xmi#p"}"#,
            r#"{"type":"RemoveReference","element":"b","end":"participantPackage"}"#,
            r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":null,"name":{"lang":"en","content":"I"},"implements":"p","implements_href":"o.xmi#p"}"#,
            r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":null,"name":{"lang":"en","content":"I"}}"#,
            r#"{"type":"AddReference","element":"b","end":"participantPackage","href":null}"#,
            r#"{"type":"AddReference","element":"b","end":"participantPackage","href":"o.xmi#p","hrefs":[]}"#,
        ] {
            refused::<Operation>(json);
        }
        let same_document = HrefError::SameDocument.to_string();
        for json in [
            r##"{"type":"AddReference","element":"b","end":"participantPackage","href":"#c"}"##,
            r##"{"type":"RemoveReference","element":"b","end":"participantPackage","href":"#c"}"##,
            r##"{"type":"SetReferences","element":"b","end":"participantPackage","ids":[],"hrefs":["o.xmi#p","#c"]}"##,
            r##"{"type":"CreateArgumentPackageInterface","parent":null,"id":null,"name":{"lang":"en","content":"I"},"implements_href":"#c"}"##,
            r##"{"type":"CreateArgumentPackageBinding","parent":null,"id":null,"name":{"lang":"en","content":"B"},"participant_packages":["a"],"participant_package_hrefs":["#c"]}"##,
        ] {
            let why = refused::<Operation>(json);
            assert!(
                why.contains(&same_document),
                "{ROW}: {json} is refused, but not with the Display of HrefError::SameDocument: {why}"
            );
        }
        // A character XML 1.0 cannot represent, which no file could hold.
        let json = r#"{"type":"AddReference","element":"b","end":"participantPackage","href":"o.xmi#a\u0001"}"#;
        let why = refused::<Operation>(json);
        assert!(
            why.contains(&HrefError::Character.to_string()),
            "{ROW}: {json} is refused, but not with the Display of HrefError::Character: {why}"
        );

        // An Href is a JSON string, checked by Href::new.
        assert_eq!(
            (
                to_json(&remote),
                from_json::<Href>("\"braking.sacm.xmi#braking_api\""),
                from_json::<Href>("\"sacm:Claim o.xmi#c\"")
                    .as_str()
                    .to_owned()
            ),
            (
                "\"braking.sacm.xmi#braking_api\"".to_owned(),
                remote.clone(),
                "sacm:Claim o.xmi#c".to_owned()
            ),
            "{ROW}: an Href as JSON"
        );
        for (json, error) in [
            ("\"o.xmi\"", HrefError::NoFragment),
            ("\" o.xmi#c\"", HrefError::WhiteSpace),
            ("\"x:Claim o.xmi#c\"", HrefError::Type),
            ("\"o.xmi#a#b\"", HrefError::HashInFragment),
            ("\"#c\"", HrefError::SameDocument),
        ] {
            let why = refused::<Href>(json);
            assert!(
                why.contains(&error.to_string()),
                "{ROW}: {json} is refused, but not with the Display of {error:?}: {why}"
            );
        }
        refused::<Href>("null");
        refused::<Href>("{\"href\":\"o.xmi#c\"}");

        // The HrefNotHeld sample of `edit-errors.json`, from a real refusal.
        let sample = r#"{"type":"HrefNotHeld","code":"SACM-CMD-001","end":"participantPackage","href":"o.xmi#x"}"#;
        let mut doc = the_case();
        let error = refuse(
            &mut doc,
            RemoveReference::href("b", participant, href("o.xmi#x")),
        );
        assert_eq!(to_json(&error), sample, "{ROW}: the JSON of HrefNotHeld");
        assert_eq!(
            from_json::<EditError>(sample),
            error,
            "{ROW}: {sample} does not read back"
        );
        assert!(
            golden_items("edit-errors.json").contains(&from_json::<serde_json::Value>(sample)),
            "{ROW}: edit-errors.json does not hold {sample}"
        );

        // An ExpressionLangString's expression: an id string, null, or
        // {"href": "…"}.
        let by_href = LangString::new("x").with_expression_href(href("terms.sacm.xmi#blade"));
        let json = r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":{"href":"terms.sacm.xmi#blade"}}"#;
        assert_eq!(to_json(&by_href), json, "{ROW}: the JSON of {by_href:?}");
        assert_eq!(
            from_json::<LangString>(json),
            by_href,
            "{ROW}: {json} does not read back"
        );
        for (value, json) in [
            (
                LangString::new("x").with_expression("blade"),
                r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":"blade"}"#,
            ),
            // An ExpressionLangString with no expression, as a file may give
            // one: no constructor builds it.
            (
                from_json::<LangString>(
                    r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":null}"#,
                ),
                r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":null}"#,
            ),
        ] {
            assert!(
                value.is_expression_lang_string() && value.expression_href().is_none(),
                "{ROW}: {json} reads with an expression href"
            );
            assert_eq!(to_json(&value), json, "{ROW}: the JSON of {value:?}");
            assert_eq!(
                from_json::<LangString>(json),
                value,
                "{ROW}: {json} does not read back"
            );
        }
        for json in [
            r##"{"type":"ExpressionLangString","lang":null,"content":"x","expression":{"href":"#t"}}"##,
            r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":{"id":"t"}}"#,
            r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":{}}"#,
            r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":{"href":"o.xmi#t","id":"t"}}"#,
            r#"{"type":"ExpressionLangString","lang":null,"content":"x","expression":{"href":null}}"#,
            r#"{"lang":null,"content":"x","expression":{"href":"o.xmi#t"}}"#,
        ] {
            refused::<LangString>(json);
        }
        let why = refused::<LangString>(
            r##"{"type":"ExpressionLangString","lang":null,"content":"x","expression":{"href":"#t"}}"##,
        );
        assert!(
            why.contains(&same_document),
            "{ROW}: an expression href of \"#t\" is refused, but not with the Display of \
             HrefError::SameDocument: {why}"
        );
        // An expression of another JSON type: the error says what it may be.
        let json = r#"{"type":"CreateClaim","parent":"arguments","id":null,"name":{"type":"ExpressionLangString","lang":null,"content":"x","expression":5}}"#;
        let why = refused::<Operation>(json);
        assert!(
            why.contains(r#"expected an id, null, or {"href": "…"}"#),
            "{ROW}: {json} is refused, but not saying what an expression may be: {why}"
        );
    }
}

// =================================================================================
// 8. A document built with every new constructor
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one document built with every constructor reads best as one sequence"
)]
fn lib_edit_020_hrefs_made_by_edits_write_and_read_back() {
    use ReferenceEnd::{CitedElement, Implements, MetaClaim, ParticipantPackage};
    let remote = |fragment: &str| href(&format!("remote.sacm.xmi#{fragment}"));
    let mut doc = Document::new();
    applied(
        &mut doc,
        CreateAssuranceCasePackage::root(en("Case")).with_id("case"),
    );
    applied(
        &mut doc,
        CreateArgumentPackage::new("case", en("Arguments")).with_id("args"),
    );
    applied(
        &mut doc,
        CreateArtifactPackage::new("case", en("Artifacts")).with_id("arts"),
    );
    applied(
        &mut doc,
        CreateTerminologyPackage::new("case", en("Terms")).with_id("terms"),
    );
    // The four interface creates, as roots and as children.
    applied(
        &mut doc,
        CreateAssuranceCasePackageInterface::root_href(en("Case interface"), remote("case"))
            .with_id("case_api"),
    );
    applied(
        &mut doc,
        CreateArgumentPackageInterface::new_href(
            "case",
            en("Argument interface"),
            href("sacm:ArgumentPackage remote.sacm.xmi#args"),
        )
        .with_id("args_api"),
    );
    applied(
        &mut doc,
        CreateArtifactPackageInterface::new_href(
            "case",
            en("Artifact interface"),
            href("remote.sacm.xmi#xpointer(id('arts'))"),
        )
        .with_id("arts_api"),
    );
    applied(
        &mut doc,
        CreateTerminologyPackageInterface::root_href(en("Terminology interface"), remote("terms"))
            .with_id("terms_api"),
    );
    // The four binding creates.
    applied(
        &mut doc,
        CreateAssuranceCasePackageBinding::new("case", en("Case binding"), ["case"])
            .with_participant_package_hrefs([remote("case_api")])
            .with_id("case_binding"),
    );
    applied(
        &mut doc,
        CreateArgumentPackageBinding::new("case", en("Argument binding"), NO_IDS)
            .with_participant_package_hrefs([remote("a"), remote("b")])
            .with_id("args_binding"),
    );
    applied(
        &mut doc,
        CreateArtifactPackageBinding::root(en("Artifact binding"), ["arts"])
            .with_participant_package_hrefs([remote("arts_api"), remote("arts_api")])
            .with_id("arts_binding"),
    );
    applied(
        &mut doc,
        CreateTerminologyPackageBinding::new("case", en("Terminology binding"), ["terms"])
            .with_participant_package_hrefs([href("#xpointer(id('terms_api'))")])
            .with_id("terms_binding"),
    );
    // A name with an expression href, by CreateClaim and by SetName.
    applied(
        &mut doc,
        CreateClaim::new(
            "args",
            LangString::new("Blade")
                .with_lang("en")
                .with_expression_href(href("sacm:Term glossary.sacm.xmi#blade")),
        )
        .with_id("c1"),
    );
    applied(&mut doc, CreateClaim::new("args", en("C2")).with_id("c2"));
    applied(
        &mut doc,
        SetName::new(
            "c2",
            LangString::new("C2").with_expression_href(remote("term")),
        ),
    );
    // The three reference edits.
    applied(
        &mut doc,
        SetReferences::new("c1", MetaClaim, ["c2"]).with_hrefs([
            remote("m1"),
            remote("m2"),
            remote("m1"),
        ]),
    );
    applied(&mut doc, AddReference::href("c1", MetaClaim, remote("m3")));
    applied(
        &mut doc,
        RemoveReference::href("c1", MetaClaim, remote("m1")),
    );
    applied(
        &mut doc,
        AddReference::href("c2", CitedElement, remote("cited")),
    );
    applied(
        &mut doc,
        AddReference::href("args_binding", ParticipantPackage, remote("c")),
    );
    applied(
        &mut doc,
        RemoveReference::href("args_binding", ParticipantPackage, remote("a")),
    );

    // What the document holds.
    assert_eq!(
        (
            hrefs(&doc, "c1", MetaClaim),
            ids(&doc, "c1", MetaClaim),
            hrefs(&doc, "c2", CitedElement),
            participants(&doc, "args_binding"),
            participants(&doc, "arts_binding"),
            hrefs(&doc, "args_api", Implements),
            hrefs(&doc, "arts_api", Implements)
        ),
        (
            owned(&["remote.sacm.xmi#m2", "remote.sacm.xmi#m3"]),
            owned(&["c2"]),
            owned(&["remote.sacm.xmi#cited"]),
            (
                Vec::new(),
                owned(&["remote.sacm.xmi#b", "remote.sacm.xmi#c"])
            ),
            (
                owned(&["arts"]),
                owned(&["remote.sacm.xmi#arts_api", "remote.sacm.xmi#arts_api"])
            ),
            owned(&["sacm:ArgumentPackage remote.sacm.xmi#args"]),
            owned(&["remote.sacm.xmi#xpointer(id('arts'))"]),
        ),
        "{ROW}: what the edits built"
    );
    assert!(
        !doc.has_preserved_content(),
        "{ROW}: the built document holds preserved content"
    );

    // It writes strictly, and reads back ==, in both modes, byte-stable.
    let text = round_trip("the built document", &doc);
    let found = lines(&text);
    for line in [
        "<implements href=\"remote.sacm.xmi#case\" />",
        "<implements href=\"sacm:ArgumentPackage remote.sacm.xmi#args\" />",
        "<implements href=\"remote.sacm.xmi#xpointer(id('arts'))\" />",
        "<implements href=\"remote.sacm.xmi#terms\" />",
        "<participantPackage xmi:idref=\"case\" />",
        "<participantPackage href=\"remote.sacm.xmi#case_api\" />",
        "<participantPackage href=\"remote.sacm.xmi#b\" />",
        "<participantPackage href=\"remote.sacm.xmi#c\" />",
        "<participantPackage href=\"#xpointer(id('terms_api'))\" />",
        "<expression href=\"sacm:Term glossary.sacm.xmi#blade\" />",
        "<expression href=\"remote.sacm.xmi#term\" />",
        "<metaClaim xmi:idref=\"c2\" />",
        "<metaClaim href=\"remote.sacm.xmi#m2\" />",
        "<metaClaim href=\"remote.sacm.xmi#m3\" />",
        "<citedElement href=\"remote.sacm.xmi#cited\" />",
    ] {
        assert!(
            found.contains(&line),
            "{ROW}: the output has no line {line}\n{text}"
        );
    }
    assert_eq!(
        found
            .iter()
            .filter(|line| **line == "<participantPackage href=\"remote.sacm.xmi#arts_api\" />")
            .count(),
        2,
        "{ROW}: the repeated participant href is not written twice\n{text}"
    );
    assert!(
        !text.contains("remote.sacm.xmi#m1") && !text.contains("remote.sacm.xmi#a\""),
        "{ROW}: a removed href is still written\n{text}"
    );
}
