//! Tests that an id named only by a reference is never generated
//! (SACM23-XMI-020; ADR 0024, ADR 0026).
//!
//! A reference may name an id that no element of the file holds. The reader
//! reserves every such id, in every end and in each of the three forms a
//! reference is written in (an attribute token, an `xmi:idref` element and
//! an `href="#id"` element), so that neither an element read without an
//! `xmi:id` nor a later create is given it. The ends are listed here from
//! ptc/22-03-13, not taken from the crate.

use sacm::edit::{self, CreateClaim};
use sacm::model::{Document, ElementId, LangString, ReferenceEnd, SacmElement};
use sacm::xmi::{self, Mode, ReadOptions};

const ROW: &str = "SACM23-XMI-020";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The id the references name and no element holds: the first id the edit
/// scheme gives a Claim.
const RESERVED: &str = "claim_1";

/// Every reference end, with a readable class that owns it (§8.2, §11.2,
/// §11.9, §11.10, §11.12, §11.13).
const ENDS: [(ReferenceEnd, &str); 9] = [
    (ReferenceEnd::CitedElement, "Claim"),
    (ReferenceEnd::AbstractForm, "Claim"),
    (ReferenceEnd::MetaClaim, "Claim"),
    (ReferenceEnd::Reasoning, "AssertedInference"),
    (ReferenceEnd::Source, "AssertedInference"),
    (ReferenceEnd::Target, "AssertedInference"),
    (ReferenceEnd::Structure, "ArgumentReasoning"),
    (ReferenceEnd::ReferencedArtifactElement, "ArtifactReference"),
    (ReferenceEnd::GroupMember, "ArgumentGroup"),
];

/// The three forms of a reference to [`RESERVED`] in `end`: as the
/// attribute and the element written inside the holder.
fn forms(end: &str) -> [(&'static str, String, String); 3] {
    [
        ("attribute", format!(" {end}=\"{RESERVED}\""), String::new()),
        (
            "xmi:idref",
            String::new(),
            format!("<{end} xmi:idref=\"{RESERVED}\"/>"),
        ),
        (
            "href",
            String::new(),
            format!("<{end} href=\"#{RESERVED}\"/>"),
        ),
    ]
}

/// The root ArgumentPackage `p` holding the element `h` of `class` with the
/// reference to [`RESERVED`], and, when `id_less` is true, a Claim without an
/// `xmi:id` before it (`before` true) or after it.
fn document(class: &str, attribute: &str, element: &str, id_less: bool, before: bool) -> String {
    let holder = format!(
        "<argumentElement xmi:type=\"sacm:{class}\" xmi:id=\"h\"{attribute}>\
         <name content=\"H\"/>{element}</argumentElement>"
    );
    let unnamed = if id_less {
        "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"Id-less\"/></argumentElement>"
    } else {
        ""
    };
    let (first, second) = if before {
        (unnamed, holder.as_str())
    } else {
        (holder.as_str(), unnamed)
    };
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
         xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\
         <name content=\"P\"/>{first}{second}</sacm:ArgumentPackage>"
    )
}

/// Reads `input` in `mode`, which the test expects to succeed.
fn read(what: &str, mode: Mode, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error}\n{input}"),
    }
}

#[test]
fn sacm23_xmi_020_an_id_named_only_by_a_reference_is_never_generated() {
    let mut checked = 0;
    for (end, class) in ENDS {
        for (form, attribute, element) in forms(end.name()) {
            for mode in MODES {
                // On read: an id-less Claim before or after the reference.
                for before in [true, false] {
                    let place = if before { "before" } else { "after" };
                    let what = format!(
                        "an id-less Claim {place} a {class} naming {RESERVED} in its {} by {form}",
                        end.name()
                    );
                    let doc = read(
                        &what,
                        mode,
                        &document(class, &attribute, &element, true, before),
                    );
                    let ids: Vec<&ElementId> = doc.elements().map(SacmElement::id).collect();
                    assert_eq!(
                        ids.len(),
                        3,
                        "{ROW}: {what}: {mode:?}: the read holds p, h and the id-less Claim: {ids:?}"
                    );
                    assert!(
                        !doc.contains(RESERVED),
                        "{ROW}: {what}: {mode:?}: the id-less Claim was given the referenced id \
                         {RESERVED}: {ids:?}"
                    );
                }
                // On create: a Claim with a generated id.
                let what = format!(
                    "CreateClaim beside a {class} naming {RESERVED} in its {} by {form}",
                    end.name()
                );
                let mut doc = read(
                    &what,
                    mode,
                    &document(class, &attribute, &element, false, true),
                );
                let created =
                    match edit::apply(&mut doc, CreateClaim::new("p", LangString::new("New"))) {
                        Ok(mutation) => mutation.created_ids().to_vec(),
                        Err(error) => {
                            panic!("{ROW}: {what}: {mode:?}: the create was refused: {error}")
                        }
                    };
                assert!(
                    created.len() == 1 && created[0].as_str() != RESERVED,
                    "{ROW}: {what}: {mode:?}: the create was given the referenced id: {created:?}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(
        checked,
        9 * 3 * 2,
        "{ROW}: every end, form and mode is checked"
    );
}
