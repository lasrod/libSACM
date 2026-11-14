//! Validation: checking a document against SACM's rules.
//!
//! A document read from a file can break SACM's rules and still load, so
//! that it can be inspected and fixed. [`validate`] reports what is wrong as
//! [`Diagnostic`]s. It never changes a document and never refuses one.
//!
//! # Checked in this release
//!
//! | Code | Row | Clause | Severity | Finding |
//! |---|---|---|---|---|
//! | SACM-MULT-001 | SACM23-VAL-018 | §8.6 | Error | a `ModelElement` has no name (`name:LangString[1]`) |
//! | SACM-MULT-001 | SACM23-VAL-010 | §8.5 | Error | a `content` or `key` has no value (`value:LangString[1..*]`), or two or more of its values share a language (each `lang` "must be unique") |
//! | SACM-MULT-001 | SACM23-VAL-010 | §8.12 | Error | a `TaggedValue` has no `key` (`key:MultiLangString[1]`) |
//! | SACM-ID-002 | SACM23-VAL-020 | XMI 2.5.1 §7.6.1 | Error | an element's id, or the `xmi:id` of its name, `content`, `key` or a `value`, is not an XML `NCName` (an `xmi:id` is an `xsd:ID`) |
//! | SACM-MULT-001 | SACM23-VAL-011 | §8.6 | Warning | a `ModelElement` has two or more descriptions (`description:Description[0..1]`) |
//! | SACM-ID-003 | SACM23-VAL-002 | §8.2 | Error | two or more elements hold the same non-empty `gid` ("unique within the scope of the model instance") |
//! | SACM-PKG-001 | SACM23-VAL-012 | §11.4 | Error | an `ArgumentPackage` nests a plain `ArgumentPackage` and also holds another argumentation element ("only allowed to contain `ArgumentPackages`") |
//! | SACM-REF-001 | SACM23-VAL-003 | the end's: §8.2, §11.2, §11.9, §11.10, §11.12 or §11.13 | Error | an id a reference end holds (`citedElement`, `abstractForm`, `metaClaim`, `reasoning`, `source`, `target`, `structure`, `referencedArtifactElement` or a group's `argumentElement`) names no element of the document |
//! | SACM-REF-002 | SACM23-VAL-004 | the end's | Error | an id a reference end holds names an element of a kind the end's declared type does not admit, such as a `metaClaim` naming an `ArgumentReasoning` |
//! | SACM-REF-002 | SACM23-VAL-016 | §11.15, §11.17 or §11.18 | Error | a source of an `AssertedEvidence`, or a source or target of an `AssertedArtifactSupport` or `AssertedArtifactContext`, is an `ArgumentAsset` but not an `ArtifactReference` |
//! | SACM-MULT-001 | SACM23-VAL-005 | §11.13 | Error | an asserted relationship has no `source` (`source:ArgumentAsset[1..*]`), or no `target` (`target:ArgumentAsset[1]`): one finding per missing end |
//! | SACM-MULT-001 | SACM23-VAL-022 | the end's: §11.2, §11.9, §11.10 or §11.13 | Warning | a multi-valued reference end of an element holds an id two or more times: one finding per repeated id |
//! | SACM-CITE-001 | SACM23-VAL-007 | §8.2 | Error | an element has a `citedElement` but its `isCitation` is not true (OCL `self.citedElement <> null implies self.isCitation = true`) |
//! | SACM-ABS-001 | SACM23-VAL-008 | §8.2 | Error | an element with an `abstractForm` is itself abstract ("+isAbstract of the `SACMElement` is false") |
//! | SACM-ABS-001 | SACM23-VAL-008 | §8.2 | Warning | the element the `abstractForm` references is not abstract, or is of another kind (both "should") |
//!
//! A `ModelElement`
//! ([`AbstractClass::ModelElement`](crate::model::AbstractClass::ModelElement))
//! is any named, identifiable element: every package, claim, asserted
//! relationship, artifact and term. The four `UtilityElement` kinds
//! (`Description`, `ImplementationConstraint`, `Note`, `TaggedValue`) have no
//! name and are not checked. An empty name (`<name/>`) is a name. To fix a
//! finding, give the element a `<name lang="…" content="…"/>` child.
//!
//! A `UtilityElement`'s `content`, and a `TaggedValue`'s `key`, is a
//! `MultiLangString` (§8.5): when present, it must hold at least one value,
//! and no two of its values may share a `lang`. Languages are compared
//! exactly: two absent ones are equal, an absent one and `""` differ, and so
//! do `en` and `EN`. A `content` that is absent gives no finding, but a
//! `TaggedValue` must have a `key`. Each finding affects the
//! `UtilityElement`, and the message names the role and the language. To fix
//! one, give the `content` or `key` one `<value lang="…" content="…"/>` per
//! language.
//!
//! ```
//! use sacm::diagnostic::Code;
//! use sacm::edit::{self, CreateArgumentPackage, CreateNote};
//! use sacm::model::{Document, LangString, MultiLangString};
//! use sacm::validation;
//!
//! let mut doc = Document::new();
//! edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
//! // A Note whose content says two things in English.
//! let content: MultiLangString = [
//!     LangString::new("Reviewed").with_lang("en"),
//!     LangString::new("Checked").with_lang("en"),
//! ]
//! .into_iter()
//! .collect();
//! edit::apply(&mut doc, CreateNote::new("p", content).with_id("n1"))?;
//!
//! let found = validation::validate(&doc);
//! assert_eq!(found.len(), 1);
//! assert_eq!(found[0].code(), Code::MULTIPLICITY_VIOLATION);
//! assert_eq!(found[0].affected()[0].as_str(), "n1");
//! # Ok::<(), sacm::edit::EditError>(())
//! ```
//!
//! Every element's id is checked, and so is the `xmi:id` of every name
//! ([`SacmElement::name_id`](crate::model::SacmElement::name_id)), `content`,
//! `key` and `value`
//! ([`SacmElement::content_id`](crate::model::SacmElement::content_id) and
//! the like), whose finding affects the element that owns it. An `NCName` starts with
//! a letter or `_` and continues with letters, digits, `-`, `.` or `_` (XML
//! 1.0 Fifth Edition and Namespaces in XML Third Edition give the full
//! character ranges); it has no spaces and no `:`.
//! [`xmi::read`](crate::xmi::read) reads an id such as `1c`, `a:b` or `a b`
//! as written, so that nothing is lost, and refuses only an empty one; it
//! reads an `NCName` with XML white space around it, such as `" c1 "`,
//! trimmed. [`edit::apply`](crate::edit::apply) never creates an id that is
//! not an `NCName`. To fix a
//! finding, give the element, or its name, `content`, `key` or `value`, an
//! id that is an `NCName`.
//!
//! The §8.6 text gives `description:Description[0..1]`, while the model
//! says `0..*`, so every description is kept and a `ModelElement` with two
//! or more is a Warning, affecting the element (D-DESC-MULT). Notes,
//! implementation constraints and tagged values are not counted. To fix one,
//! merge the descriptions into one.
//!
//! A `gid` (§8.2) is compared across the whole document, on elements of
//! every kind. Each value two or more elements hold is one finding,
//! affecting every holder in document order. An absent or empty (`""`) gid
//! is not compared, and values are compared exactly, letter case and white
//! space included (D-GID-EMPTY). A gid is not an `xmi:id`, so a gid equal
//! to an id is no finding. To fix one, give each element its own gid.
//!
//! An `ArgumentPackage` that nests a plain `ArgumentPackage` may hold only
//! `ArgumentPackage`s (§11.4). Each other `argumentElement` child, such as a
//! `Claim`, is one finding, affecting the package, then the child. An
//! `ArgumentPackageInterface` or `ArgumentPackageBinding` child neither
//! triggers the rule nor is reported, `UtilityElement` children are not
//! counted, and only a plain `ArgumentPackage` is checked as an owner
//! (D-PKG-HOMOG). Each package is checked for its own children only. To fix
//! one, move the other elements into a nested package.
//!
//! A reference is held as the id it names (ADR 0026). Each distinct id a
//! reference end holds that names no element of the document is one
//! SACM-REF-001 finding per element and end, with the end's clause, affecting
//! the element, then the id; an id a multi-valued end holds twice is one
//! finding. An id absent from the document dangles, and so does
//! the `xmi:id` of a `name`, `content`, `key` or `value`, which identifies no
//! element. A reference to any element, the element itself included,
//! resolves. To fix one, point it at an element, or remove it.
//!
//! Each end has a declared type (ptc/22-03-13, on which the text agrees):
//! `SACMElement` for `citedElement` and `abstractForm`, `Claim` for
//! `metaClaim`, `ArgumentReasoning` for `reasoning`, `ArgumentAsset` for
//! `source` and `target`, `ArgumentPackage` for `structure`,
//! `ArtifactElement` for `referencedArtifactElement` (every kind but the
//! `UtilityElement`s)
//! and `ArgumentationElement` for a group's `argumentElement`. Each distinct
//! id an end holds that resolves to an element of a kind the type does not
//! admit is one SACM-REF-002 Error per element and end (SACM23-VAL-004),
//! with the end's clause, affecting the element, then the id. A dangling id
//! is only SACM-REF-001. Besides, §11.15 requires the sources of an
//! `AssertedEvidence`, and §11.17 and §11.18 the sources and target of an
//! `AssertedArtifactSupport` and an `AssertedArtifactContext`, to be
//! `ArtifactReference`s: each distinct other `ArgumentAsset` there is one
//! SACM-REF-002 Error with that clause (SACM23-VAL-016). An id that the
//! generic rules already report is not reported again, so each wrong
//! reference is one finding. To fix one, point the reference at an element
//! of the right kind.
//!
//! A multi-valued end (`metaClaim`, `source`, `referencedArtifactElement`, a
//! group's `argumentElement`) holds its ids in order, repeats included, as
//! read (ADR 0027). SACM's ends hold each id once, so each id one end of one
//! element holds two or more times is one SACM-MULT-001 Warning
//! (SACM23-VAL-022), with the end's clause, affecting the element, then the
//! id. It is reported whatever else applies to the id, and a strict save
//! still writes the repeat. To fix one, remove the repeats.
//!
//! An asserted relationship needs at least one `source` and one `target`
//! (§11.13). One read without them is kept as read, and each missing end is
//! one SACM-MULT-001 Error, affecting the relationship, so a strict
//! [`xmi::write`](crate::xmi::write) refuses it. To fix one, give it the
//! missing end.
//!
//! §8.2 asks an element with a `citedElement` to be a citation: one without
//! `isCitation` true is a SACM-CITE-001 Error, affecting it, whether or not
//! the cited id resolves; `isCitation` true alone is no finding. An element
//! with an `abstractForm` must be concrete: one with `isAbstract` true is a
//! SACM-ABS-001 Error, affecting it. When the `abstractForm` names an element,
//! that element should be abstract and of the same kind: there is one
//! SACM-ABS-001 Warning when it is not abstract and another when it is of
//! another kind, each affecting the element, then the referenced element. An `abstractForm`
//! that names no element gives only SACM-REF-001.
//!
//! ```
//! use sacm::diagnostic::{Code, Severity};
//! use sacm::validation;
//! use sacm::xmi::{self, ReadOptions};
//!
//! let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
//!     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
//!   <name content="Arguments"/>
//!   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" abstractForm="c2">
//!     <name content="The brakes are safe"/>
//!   </argumentElement>
//!   <argumentElement xmi:type="sacm:Claim" xmi:id="c2">
//!     <name content="The pump is safe"/>
//!   </argumentElement>
//! </sacm:ArgumentPackage>"#;
//! let document = xmi::read(input, &ReadOptions::default())?.into_document();
//! // c2 is not abstract, so c1 should not conform to it.
//! let found = validation::validate(&document);
//! assert_eq!(found.len(), 1);
//! assert_eq!((found[0].code(), found[0].severity()), (Code::ABSTRACT_FORM_INVALID, Severity::Warning));
//! let affected: Vec<&str> = found[0].affected().iter().map(|id| id.as_str()).collect();
//! assert_eq!(affected, ["c1", "c2"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The result is sorted by [`Diagnostic`]'s order. Its diagnostics have no
//! [`location`](Diagnostic::location), because a [`Document`] records no
//! source positions.
//!
//! Later releases add rules, so a document with no findings today may have
//! findings later. A code's meaning never changes.
//!
//! A strict [`xmi::write`](crate::xmi::write), the default, runs `validate`
//! and refuses a document it reports an Error for, as
//! [`WriteError::Invalid`](crate::xmi::WriteError::Invalid) (ADR 0022).
//!
//! To report reading and validation together, copy
//! [`ReadOutput::diagnostics`](crate::xmi::ReadOutput::diagnostics) into a
//! `Vec`, extend it with the result and sort it again, as below. The reader
//! reports an element without an `xmi:id` (SACM-XMI-004), an `xmi:id` read
//! trimmed (SACM-ID-004), an `xsi:type` (SACM-XMI-010) and an `xmi:version`
//! (SACM-XMI-011); it refuses what it cannot represent.
//!
//! # Examples
//!
//! ```
//! use sacm::diagnostic::{self, Code};
//! use sacm::validation;
//! use sacm::xmi::{self, ReadOptions};
//!
//! let input = br#"<sacm:ArgumentPackage
//!     xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
//!     xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
//!     xmi:id="arguments"/>"#;
//! let output = xmi::read(input, &ReadOptions::default())?;
//!
//! let found = validation::validate(output.document());
//! assert_eq!(found.len(), 1);
//! assert_eq!(found[0].code(), Code::MULTIPLICITY_VIOLATION);
//! assert_eq!(found[0].affected()[0].as_str(), "arguments");
//! assert!(diagnostic::has_errors(&found));
//!
//! // Reading and validation, together and in order.
//! let mut all = output.diagnostics().to_vec();
//! all.extend(found);
//! all.sort();
//! assert_eq!(all.len(), 1);
//! assert!(diagnostic::has_errors(&all));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod descriptions;
mod gid;
mod homogeneity;
mod ids;
mod multi_lang;
mod names;
mod references;
mod relationships;
mod repeats;

use crate::diagnostic::Diagnostic;
use crate::model::Document;

/// A rule: pushes its findings about the document.
type Rule = fn(&Document, &mut Vec<Diagnostic>);

/// Every rule, run in turn by [`validate`].
const RULES: &[Rule] = &[
    names::missing_name,
    multi_lang::multi_lang_strings,
    ids::invalid_id,
    gid::duplicate_gid,
    descriptions::repeated_description,
    homogeneity::argument_package_homogeneity,
    references::dangling_references,
    references::wrongly_typed_references,
    references::citation,
    references::abstract_form,
    relationships::relationship_ends,
    relationships::artifact_reference_kinds,
    repeats::repeated_references,
];

/// Checks `document` against the SACM rules this release knows and returns
/// the findings, sorted by [`Diagnostic`]'s order. An empty result means no
/// rule found anything, not that the document conforms in full.
///
/// The document is not changed. See the [module docs](self) for what is
/// checked.
///
/// A later minor release may add an Error rule. That is not a breaking
/// change, even though a strict [`xmi::write`](crate::xmi::write) then
/// refuses a document it wrote before; the release notes list each such rule
/// (ADR 0022).
///
/// An empty name is a name: `<name/>` gives no finding.
///
/// ```
/// use sacm::validation;
/// use sacm::xmi::{self, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage
///     xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
///     xmi:id="arguments"><name/></sacm:ArgumentPackage>"#;
/// let document = xmi::read(input, &ReadOptions::default())?.into_document();
/// assert!(validation::validate(&document).is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn validate(document: &Document) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    for rule in RULES {
        rule(document, &mut found);
    }
    found.sort();
    found
}
