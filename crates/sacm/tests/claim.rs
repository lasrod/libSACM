//! Tests for Claim and the Assertion attribute `assertionDeclaration`
//! (SACM23-ARG-002, formal/23-05-08 §11.10 and §11.11, ADR 0011).
//!
//! `assertionDeclaration: AssertionDeclaration [1] = asserted` is declared on
//! Assertion, so every element of the six Assertion kinds (Claim and the five
//! asserted-relationship kinds) holds exactly one declaration: `asserted` until
//! another is set. Every other kind has none. It is read through the stable
//! `SacmElement::assertion_declaration` and set through the `test-support`
//! seam `Document::set_assertion_declaration`, which refuses an unknown id or
//! a kind that is not an Assertion with a `SetAssertionDeclarationError`.
//!
//! The crate dev-depends on itself with `test-support`, so these tests always
//! see the seam.
//!
//! Not checked here: item 7's `#[non_exhaustive]`. A test cannot show that an
//! exhaustive `match` fails to compile outside the crate; the verifier checks
//! the attribute in the source and the rustdoc.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Debug, Display};

use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, SacmElement,
    SetAssertionDeclarationError,
};

/// One mebibyte.
const MIB: usize = 1 << 20;

/// The six Assertion kinds, spelled out (§11.10, §11.11, §11.13 to §11.17).
const ASSERTION_KINDS: [ElementKind; 6] = [
    ElementKind::Claim,
    ElementKind::AssertedInference,
    ElementKind::AssertedEvidence,
    ElementKind::AssertedContext,
    ElementKind::AssertedArtifactSupport,
    ElementKind::AssertedArtifactContext,
];

/// The six literals, in model order.
const LITERALS: [AssertionDeclaration; 6] = [
    AssertionDeclaration::Asserted,
    AssertionDeclaration::NeedsSupport,
    AssertionDeclaration::Assumed,
    AssertionDeclaration::Axiomatic,
    AssertionDeclaration::Defeated,
    AssertionDeclaration::AsCited,
];

/// Inserts an element, failing the test if the seam refuses it.
fn add(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    let result = doc.insert(parent.as_ref(), kind, ElementId::from(id), None);
    assert_eq!(
        result,
        Ok(()),
        "SACM23-ARG-002: setup could not insert the {kind} of {} bytes under {parent:?}",
        id.len()
    );
}

/// Looks up an element that the test expects to exist.
fn view<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(element) = doc.get(id) else {
        panic!("SACM23-ARG-002: {id:?} is missing from the document");
    };
    element
}

/// The declaration of the element `id`, which must exist.
fn declaration(doc: &Document, id: &str) -> Option<AssertionDeclaration> {
    view(doc, id).assertion_declaration()
}

/// Sets a declaration that the test expects the seam to accept.
fn set(doc: &mut Document, id: &str, value: AssertionDeclaration) {
    assert_eq!(
        doc.set_assertion_declaration(&ElementId::from(id), value),
        Ok(()),
        "SACM23-ARG-002: setting {value} on {id:?} was refused"
    );
}

/// Every element's id, kind, parent and declaration, in document order: what
/// a refused set must leave as it was.
type Snapshot = Vec<(
    String,
    ElementKind,
    Option<String>,
    Option<AssertionDeclaration>,
)>;

fn snapshot(doc: &Document) -> Snapshot {
    doc.elements()
        .map(|element| {
            (
                element.id().to_string(),
                element.kind(),
                element.parent().map(|parent| parent.id().to_string()),
                element.assertion_declaration(),
            )
        })
        .collect()
}

/// A root `ArgumentPackage` "arguments" with Claims c1 and c2, an
/// `ArgumentReasoning` r1, an `ArtifactReference` a1, a nested
/// `ArgumentPackage` p1 and a Note n1 under c1. c2 is set to `defeated`, so
/// a refusal that reset declarations would show.
fn argument_document() -> Document {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    for (kind, id) in [
        (ElementKind::Claim, "c1"),
        (ElementKind::Claim, "c2"),
        (ElementKind::ArgumentReasoning, "r1"),
        (ElementKind::ArtifactReference, "a1"),
        (ElementKind::ArgumentPackage, "p1"),
    ] {
        add(&mut doc, Some("arguments"), kind, id);
    }
    add(&mut doc, Some("c1"), ElementKind::Note, "n1");
    set(&mut doc, "c2", AssertionDeclaration::Defeated);
    doc
}

/// Attempts a set that the test expects to be refused, checks that the
/// document is unchanged, and returns why.
fn refusal(doc: &mut Document, id: &str) -> SetAssertionDeclarationError {
    let before = snapshot(doc);
    let Err(error) =
        doc.set_assertion_declaration(&ElementId::from(id), AssertionDeclaration::Axiomatic)
    else {
        panic!(
            "SACM23-ARG-002: setting a declaration on an id of {} bytes was accepted",
            id.len()
        );
    };
    assert!(
        snapshot(doc) == before,
        "SACM23-ARG-002: a refused set on an id of {} bytes changed the document",
        id.len()
    );
    error
}

// --- 1. Default --------------------------------------------------------------------

#[test]
fn sacm23_arg_002_a_claim_is_asserted_by_default() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    add(&mut doc, Some("arguments"), ElementKind::Claim, "c1");
    assert_eq!(
        declaration(&doc, "c1"),
        Some(AssertionDeclaration::Asserted),
        "SACM23-ARG-002: a new Claim is not asserted (§11.10 \"= asserted\")"
    );
    assert_eq!(
        declaration(&doc, "c1"),
        Some(AssertionDeclaration::default()),
        "SACM23-ARG-002: a new Claim's declaration is not AssertionDeclaration::default()"
    );
}

// --- 2. Set and replace -------------------------------------------------------------

#[test]
fn sacm23_arg_002_each_literal_set_on_a_claim_reads_back() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    add(&mut doc, Some("arguments"), ElementKind::Claim, "c1");
    // Asserted last, so the set back to the default is a set too.
    for value in LITERALS.iter().rev().copied() {
        set(&mut doc, "c1", value);
        assert_eq!(
            declaration(&doc, "c1"),
            Some(value),
            "SACM23-ARG-002: {value} set on a Claim does not read back"
        );
    }
}

#[test]
fn sacm23_arg_002_a_later_set_replaces_an_earlier_one() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    add(&mut doc, Some("arguments"), ElementKind::Claim, "c1");
    set(&mut doc, "c1", AssertionDeclaration::NeedsSupport);
    set(&mut doc, "c1", AssertionDeclaration::Asserted);
    assert_eq!(
        declaration(&doc, "c1"),
        Some(AssertionDeclaration::Asserted),
        "SACM23-ARG-002: needsSupport then asserted does not read asserted"
    );
    // Every ordered pair of distinct literals: the second wins.
    for first in LITERALS {
        for second in LITERALS {
            set(&mut doc, "c1", first);
            set(&mut doc, "c1", second);
            assert_eq!(
                declaration(&doc, "c1"),
                Some(second),
                "SACM23-ARG-002: {first} then {second} does not read {second}"
            );
        }
    }
}

// --- 3. Exactly the Assertion kinds -------------------------------------------------

/// A document with one element of every kind, each under the ArtifactPackage
/// root "pkg" (which admits every `ModelElement` kind, ADR 0014 and LIB-API-008),
/// with the id `e_<kind>`.
fn every_kind_document() -> Document {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArtifactPackage, "pkg");
    for &kind in ElementKind::ALL {
        add(&mut doc, Some("pkg"), kind, &format!("e_{kind}"));
    }
    doc
}

#[test]
fn sacm23_arg_002_the_assertion_kinds_are_exactly_the_six() {
    let spelled_out: BTreeSet<ElementKind> = ASSERTION_KINDS.into_iter().collect();
    let by_class: BTreeSet<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::Assertion))
        .collect();
    assert_eq!(
        by_class, spelled_out,
        "SACM23-ARG-002: the kinds that are an Assertion are not Claim and the five Asserted* kinds"
    );
}

#[test]
fn sacm23_arg_002_exactly_the_assertion_kinds_report_a_declaration() {
    let doc = every_kind_document();
    for &kind in ElementKind::ALL {
        let expected = ASSERTION_KINDS
            .contains(&kind)
            .then_some(AssertionDeclaration::Asserted);
        assert_eq!(
            declaration(&doc, &format!("e_{kind}")),
            expected,
            "SACM23-ARG-002: a new {kind} does not report {expected:?}"
        );
    }
    assert_eq!(
        declaration(&doc, "pkg"),
        None,
        "SACM23-ARG-002: the root ArtifactPackage reports a declaration"
    );
}

#[test]
fn sacm23_arg_002_argument_package_reasoning_reference_and_note_report_none() {
    let doc = argument_document();
    for id in ["arguments", "p1", "r1", "a1", "n1"] {
        assert_eq!(
            declaration(&doc, id),
            None,
            "SACM23-ARG-002: the {} {id:?} reports a declaration",
            view(&doc, id).kind()
        );
    }
}

#[test]
fn sacm23_arg_002_every_assertion_kind_accepts_all_six_literals() {
    let mut doc = every_kind_document();
    for kind in ASSERTION_KINDS {
        let id = format!("e_{kind}");
        for value in LITERALS.iter().rev().copied() {
            set(&mut doc, &id, value);
            assert_eq!(
                declaration(&doc, &id),
                Some(value),
                "SACM23-ARG-002: {value} set on a {kind} does not read back"
            );
        }
    }
}

// --- 4. Refusals --------------------------------------------------------------------

#[test]
fn sacm23_arg_002_the_setter_refuses_an_unknown_id() {
    let mut doc = argument_document();
    assert_eq!(
        refusal(&mut doc, "c9"),
        SetAssertionDeclarationError::UnknownElement(ElementId::from("c9")),
        "SACM23-ARG-002: setting on an unknown id is not UnknownElement"
    );
}

#[test]
fn sacm23_arg_002_the_setter_refuses_a_kind_that_is_not_an_assertion() {
    let mut doc = argument_document();
    for (id, kind) in [
        ("r1", ElementKind::ArgumentReasoning),
        ("a1", ElementKind::ArtifactReference),
        ("arguments", ElementKind::ArgumentPackage),
        ("p1", ElementKind::ArgumentPackage),
        ("n1", ElementKind::Note),
    ] {
        assert_eq!(
            refusal(&mut doc, id),
            SetAssertionDeclarationError::NotAnAssertion(ElementId::from(id), kind),
            "SACM23-ARG-002: setting on the {kind} {id:?} is not NotAnAssertion"
        );
        assert_eq!(
            declaration(&doc, id),
            None,
            "SACM23-ARG-002: a refused set gave the {kind} {id:?} a declaration"
        );
    }
    assert_eq!(
        (declaration(&doc, "c1"), declaration(&doc, "c2")),
        (
            Some(AssertionDeclaration::Asserted),
            Some(AssertionDeclaration::Defeated)
        ),
        "SACM23-ARG-002: refused sets changed the Claims' declarations"
    );
}

#[test]
fn sacm23_arg_002_the_setter_refuses_every_kind_that_is_not_an_assertion() {
    let mut doc = every_kind_document();
    for &kind in ElementKind::ALL {
        if ASSERTION_KINDS.contains(&kind) {
            continue;
        }
        let id = format!("e_{kind}");
        assert_eq!(
            refusal(&mut doc, &id),
            SetAssertionDeclarationError::NotAnAssertion(ElementId::from(id.as_str()), kind),
            "SACM23-ARG-002: setting on a {kind} is not NotAnAssertion"
        );
    }
}

// --- 5. Independence -----------------------------------------------------------------

#[test]
fn sacm23_arg_002_declarations_are_held_per_element_and_per_document() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    add(&mut doc, Some("arguments"), ElementKind::Claim, "c1");
    add(&mut doc, Some("arguments"), ElementKind::Claim, "c2");

    set(&mut doc, "c1", AssertionDeclaration::Defeated);
    assert_eq!(
        (declaration(&doc, "c1"), declaration(&doc, "c2")),
        (
            Some(AssertionDeclaration::Defeated),
            Some(AssertionDeclaration::Asserted)
        ),
        "SACM23-ARG-002: setting defeated on c1 did not leave its sibling c2 asserted"
    );

    let mut copy = doc.clone();
    assert_eq!(
        declaration(&copy, "c1"),
        Some(AssertionDeclaration::Defeated),
        "SACM23-ARG-002: a clone did not keep c1 defeated"
    );
    set(&mut copy, "c1", AssertionDeclaration::Axiomatic);
    assert_eq!(
        (declaration(&copy, "c1"), declaration(&doc, "c1")),
        (
            Some(AssertionDeclaration::Axiomatic),
            Some(AssertionDeclaration::Defeated)
        ),
        "SACM23-ARG-002: setting axiomatic on the clone's c1 changed the original's"
    );
    assert_eq!(
        declaration(&copy, "c2"),
        Some(AssertionDeclaration::Asserted),
        "SACM23-ARG-002: the clone's c2 is not asserted"
    );
}

// --- 6. The kind, not the container ---------------------------------------------------

#[test]
fn sacm23_arg_002_a_claim_in_an_artifact_package_holds_a_declaration() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArtifactPackage, "evidence");
    add(&mut doc, Some("evidence"), ElementKind::Claim, "c1");
    assert_eq!(
        declaration(&doc, "c1"),
        Some(AssertionDeclaration::Asserted),
        "SACM23-ARG-002: a Claim in an ArtifactPackage is not asserted by default (ADR 0014)"
    );
    for value in LITERALS.iter().rev().copied() {
        set(&mut doc, "c1", value);
        assert_eq!(
            declaration(&doc, "c1"),
            Some(value),
            "SACM23-ARG-002: {value} set on a Claim in an ArtifactPackage does not read back"
        );
    }
}

// --- 7. The error type -----------------------------------------------------------------

fn assert_error_type<T: Error + Clone + Eq + Debug + Display + Send + Sync + 'static>() {}

/// Every `SetAssertionDeclarationError` variant, each produced by the setter.
fn every_error() -> Vec<(&'static str, SetAssertionDeclarationError)> {
    let mut doc = argument_document();
    vec![
        ("UnknownElement", refusal(&mut doc, "c9")),
        ("NotAnAssertion", refusal(&mut doc, "r1")),
    ]
}

/// Checks the ADR 0011 message rules: non-empty, one line, starting lowercase
/// and not ending in `.`, `!` or `:`.
fn assert_message_rules(what: &str, message: &str) {
    assert!(
        !message.is_empty(),
        "SACM23-ARG-002: the {what} message is empty"
    );
    assert!(
        !message.contains(['\n', '\r']),
        "SACM23-ARG-002: the {what} message is not one line: {message:?}"
    );
    assert!(
        message.chars().next().is_some_and(char::is_lowercase),
        "SACM23-ARG-002: the {what} message does not start lowercase: {message:?}"
    );
    assert!(
        !message.ends_with(['.', '!', ':']),
        "SACM23-ARG-002: the {what} message ends in punctuation: {message:?}"
    );
}

#[test]
fn sacm23_arg_002_set_error_is_a_clone_eq_send_sync_static_error() {
    // Checked statically: this does not compile unless the bounds hold.
    assert_error_type::<SetAssertionDeclarationError>();
}

#[test]
fn sacm23_arg_002_set_error_boxes_and_downcasts_as_a_send_sync_error() {
    for (variant, error) in every_error() {
        let boxed: Box<dyn Error + Send + Sync + 'static> = Box::new(error.clone());
        assert_eq!(
            boxed.to_string(),
            error.to_string(),
            "SACM23-ARG-002: boxing {variant} changed its message"
        );
        assert_eq!(
            boxed.downcast_ref::<SetAssertionDeclarationError>(),
            Some(&error),
            "SACM23-ARG-002: a boxed {variant} does not downcast back to itself"
        );
    }
}

#[test]
fn sacm23_arg_002_set_error_debug_names_the_variant() {
    for (variant, error) in every_error() {
        let debug = format!("{error:?}");
        assert!(
            debug.starts_with(variant),
            "SACM23-ARG-002: Debug of {variant} does not name it: {debug}"
        );
    }
}

#[test]
fn sacm23_arg_002_set_error_display_is_exact() {
    let messages: Vec<String> = every_error()
        .into_iter()
        .map(|(_, error)| error.to_string())
        .collect();
    assert_eq!(
        messages,
        [
            "the document has no element \"c9\"",
            "element \"r1\" of kind ArgumentReasoning is not an Assertion",
        ],
        "SACM23-ARG-002: the messages are not the designed ones"
    );
}

#[test]
fn sacm23_arg_002_set_error_messages_follow_the_message_rules() {
    for (variant, error) in every_error() {
        assert_message_rules(variant, &error.to_string());
    }
}

/// Ids that would break the message rules if a message ended with the id
/// unquoted, or started with it.
const PUNCTUATED_IDS: [&str; 7] = [
    "Claim.",
    "claim!",
    "claim:",
    "UPPER",
    "ünïcödé-標識",
    "a\0b",
    " padded ",
];

/// Ids that hold line breaks: an id read from a file may.
const LINE_BREAK_IDS: [&str; 4] = ["claim\ntop", "claim\r\ntop", "trailing\n", "\rleading"];

#[test]
fn sacm23_arg_002_set_error_messages_quote_awkward_ids_on_one_line() {
    for id in PUNCTUATED_IDS.into_iter().chain(LINE_BREAK_IDS) {
        let quoted = format!("{id:?}");

        let unknown = refusal(&mut argument_document(), id);
        assert_eq!(
            unknown,
            SetAssertionDeclarationError::UnknownElement(ElementId::from(id)),
            "SACM23-ARG-002: setting on a missing {id:?} is not UnknownElement"
        );
        let message = unknown.to_string();
        assert_message_rules(&format!("UnknownElement({id:?})"), &message);
        assert_eq!(
            message,
            format!("the document has no element {quoted}"),
            "SACM23-ARG-002: UnknownElement({id:?}) does not quote and escape the id"
        );

        let mut doc = Document::new();
        add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
        add(
            &mut doc,
            Some("arguments"),
            ElementKind::ArgumentReasoning,
            id,
        );
        let not_an_assertion = refusal(&mut doc, id);
        assert_eq!(
            not_an_assertion,
            SetAssertionDeclarationError::NotAnAssertion(
                ElementId::from(id),
                ElementKind::ArgumentReasoning
            ),
            "SACM23-ARG-002: setting on the ArgumentReasoning {id:?} is not NotAnAssertion"
        );
        let message = not_an_assertion.to_string();
        assert_message_rules(&format!("NotAnAssertion({id:?})"), &message);
        assert_eq!(
            message,
            format!("element {quoted} of kind ArgumentReasoning is not an Assertion"),
            "SACM23-ARG-002: NotAnAssertion({id:?}) does not quote and escape the id"
        );
    }
}

#[test]
fn sacm23_arg_002_set_error_has_no_source() {
    for (variant, error) in every_error() {
        assert!(
            error.source().is_none(),
            "SACM23-ARG-002: {variant} wraps no cause but has a source"
        );
    }
}

/// Unknown ids that are empty, NUL-bearing or non-ASCII, or that differ from
/// an existing id ("c1") only slightly.
const HOSTILE_IDS: [&str; 12] = [
    "",
    "\0",
    "c1\0",
    "\0c1",
    "a\0b",
    "Ünïcödé-ид-標識",
    "\u{FEFF}c1",
    "e\u{301}",
    "🛡️",
    "\u{10FFFF}",
    "C1",
    " c1",
];

#[test]
fn sacm23_arg_002_the_setter_refuses_hostile_unknown_ids_without_panicking() {
    let mut doc = argument_document();
    for id in HOSTILE_IDS {
        let error = refusal(&mut doc, id);
        assert_eq!(
            error,
            SetAssertionDeclarationError::UnknownElement(ElementId::from(id)),
            "SACM23-ARG-002: setting on the unknown {id:?} is not UnknownElement"
        );
        assert_message_rules(&format!("UnknownElement({id:?})"), &error.to_string());
    }
    assert_eq!(
        declaration(&doc, "c1"),
        Some(AssertionDeclaration::Asserted),
        "SACM23-ARG-002: refused hostile ids changed c1"
    );
}

#[test]
fn sacm23_arg_002_the_setter_refuses_a_one_mebibyte_unknown_id() {
    let mut doc = argument_document();
    let big = "c".repeat(MIB);
    let error = refusal(&mut doc, &big);
    assert_eq!(
        error,
        SetAssertionDeclarationError::UnknownElement(ElementId::from(big.as_str())),
        "SACM23-ARG-002: setting on a 1 MiB unknown id is not UnknownElement"
    );
    let message = error.to_string();
    assert!(
        message.contains(&big),
        "SACM23-ARG-002: the UnknownElement message does not name the 1 MiB id"
    );
    assert_message_rules("UnknownElement(1 MiB)", &message);
}

#[test]
fn sacm23_arg_002_the_setter_accepts_a_one_mebibyte_claim_id() {
    // The id length does not matter to a known Claim either.
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    let big = "c".repeat(MIB);
    add(&mut doc, Some("arguments"), ElementKind::Claim, &big);
    set(&mut doc, &big, AssertionDeclaration::AsCited);
    assert_eq!(
        declaration(&doc, &big),
        Some(AssertionDeclaration::AsCited),
        "SACM23-ARG-002: asCited set on a Claim with a 1 MiB id does not read back"
    );
    assert_eq!(
        refusal(&mut doc, &big[..MIB - 1]),
        SetAssertionDeclarationError::UnknownElement(ElementId::from(&big[..MIB - 1])),
        "SACM23-ARG-002: the prefix of a 1 MiB id is not an unknown element"
    );
}
