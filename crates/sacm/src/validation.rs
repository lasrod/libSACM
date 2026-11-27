//! Check a document against the SACM 2.3 rules.
//!
//! A document read from a file can break SACM's rules and still load, so that it can be
//! inspected and fixed. [`validate`] is the one entry point: it reports what is wrong
//! as [`Diagnostic`]s.
//!
//! # Rules
//!
//! - **Read-only.** `validate` never changes a document and never refuses one.
//! - **Sorted.** The result is sorted by [`Diagnostic`]'s order.
//! - **No locations.** Its diagnostics have no [`location`](Diagnostic::location),
//!   because a [`Document`] records no source positions.
//! - **Rules are added.** Later releases add rules, so a document with no findings today
//!   may have findings later. A code's meaning never changes.
//! - **With reading.** To report reading and validation together, copy
//!   [`ReadOutput::diagnostics`](crate::xmi::ReadOutput::diagnostics) into a `Vec`,
//!   extend it with the result and sort it again, as in the [example](#examples).
//! - **The reader reports its own findings,** which `validate` does not repeat, such as an
//!   element without an `xmi:id` (SACM-XMI-004), an `xmi:id` read trimmed (SACM-ID-004), an
//!   `xsi:type` (SACM-XMI-010) and an `xmi:version` (SACM-XMI-011); see
//!   [Diagnostics](crate::xmi::read#diagnostics). A strict read refuses what it cannot represent.
//!
//! # Checked in this release
//!
//! | Code | Row | Clause | Severity | Finding |
//! |---|---|---|---|---|
//! | SACM-MULT-001 | SACM23-VAL-018 | §8.6 | Error | a `ModelElement` has no name (`name:LangString[1]`) |
//! | SACM-MULT-001 | SACM23-VAL-010 | §8.5 | Error | a `content`, `key` or `location` (§12.10) has no value (`value:LangString[1..*]`), or two or more of its values share a language (each `lang` "must be unique") |
//! | SACM-MULT-001 | SACM23-VAL-010 | §8.12 | Error | a `TaggedValue` has no `key` (`key:MultiLangString[1]`) |
//! | SACM-MULT-001 | SACM23-TERM-003 | §10.9 | Error | a `Term` or an `Expression` has no `value` (`value:String[1]`) |
//! | SACM-MULT-001 | SACM23-BASE-009 | §8.4 | Error | an `ExpressionLangString`, a name or a value of a `content`, `key` or `location`, has no `expression` (`expression:ExpressionElement[1]`) |
//! | SACM-EXPR-001 | SACM23-VAL-009 | §8.4 | Warning | an `ExpressionLangString` has an `expression` and a content that is not empty ("If expression is not empty, then +content should be empty"); an absent content and `""` are empty |
//! | SACM-ID-002 | SACM23-VAL-020 | XMI 2.5.1 §7.6.1 | Error | an element's id, or the `xmi:id` of its name, `content`, `key`, `location` or a `value`, is not an XML `NCName` (an `xmi:id` is an `xsd:ID`) |
//! | SACM-MULT-001 | SACM23-VAL-011 | §8.6 | Warning | a `ModelElement` has two or more descriptions (`description:Description[0..1]`) |
//! | SACM-ID-003 | SACM23-VAL-002 | §8.2 | Error | two or more elements hold the same non-empty `gid` ("unique within the scope of the model instance") |
//! | SACM-PKG-001 | SACM23-VAL-012 | §11.4 | Error | an `ArgumentPackage` nests a plain `ArgumentPackage` and also holds another argumentation element ("only allowed to contain `ArgumentPackages`") |
//! | SACM-REF-001 | SACM23-VAL-003 | the end's: §8.2, §8.4 (an `ExpressionLangString`'s `expression`), §10.3, §10.8 (a Category's `category`), §10.9, §10.10, §10.11, §11.2, §11.9, §11.10, §11.12, §11.13, §12.3 or §12.14 | Error | an id a reference end holds (`citedElement`, `abstractForm`, `metaClaim`, `reasoning`, `source`, `target`, `structure`, `referencedArtifactElement`, a group's `argumentElement`, `artifactElement` or `terminologyElement`, an `ArtifactAssetRelationship`'s `source` or `target`, `category`, an Expression's `element`, a Term's `origin`, or the `expression` of a name or text) names no element of the document |
//! | SACM-REF-003 | LIB-COMPAT-003 | the end's | Warning | an id a reference end holds is the `xmi:id` of an element the document holds only as preserved content (ADR 0033), so its type cannot be checked |
//! | SACM-REF-002 | SACM23-VAL-004 | the end's | Error | an id a reference end holds names an element of a kind the end's declared type does not admit, such as a `metaClaim` naming an `ArgumentReasoning` |
//! | SACM-REF-002 | SACM23-VAL-016 | §11.15, §11.17 or §11.18 | Error | a source of an `AssertedEvidence`, or a source or target of an `AssertedArtifactSupport` or `AssertedArtifactContext`, is an `ArgumentAsset` but not an `ArtifactReference` |
//! | SACM-MULT-001 | SACM23-VAL-005 | §11.13 | Error | an asserted relationship has no `source` (`source:ArgumentAsset[1..*]`), or no `target` (`target:ArgumentAsset[1]`): one finding per missing end |
//! | SACM-MULT-001 | SACM23-ART-010 | §12.14 | Error | an `ArtifactAssetRelationship` has no `source` (`source:ArtifactAsset[1..*]`), or no `target` (`target:ArtifactAsset[1..*]`): one finding per missing end |
//! | SACM-MULT-001 | SACM23-VAL-022 | the end's: §10.3, §10.8, §10.9, §10.10, §11.2, §11.9, §11.10, §11.13, §12.3 or §12.14 | Warning | a multi-valued reference end of an element holds an id two or more times: one finding per repeated id |
//! | SACM-CITE-001 | SACM23-VAL-007 | §8.2 | Error | an element has a `citedElement` but its `isCitation` is not true (OCL `self.citedElement <> null implies self.isCitation = true`) |
//! | SACM-ABS-001 | SACM23-VAL-008 | §8.2 | Error | an element with an `abstractForm` is itself abstract ("+isAbstract of the `SACMElement` is false") |
//! | SACM-ABS-001 | SACM23-VAL-008 | §8.2 | Warning | the element the `abstractForm` references is not abstract, or is of another kind (both "should") |
//! | SACM-ABS-001 | SACM23-VAL-021 | §8.6 | Error | a `ModelElement` with one or more `implementationConstraint`s is not abstract (OCL `implementationConstraint->size() > 0 implies isAbstract = true`) |
//! | SACM-ABS-001 | SACM23-VAL-017 | §10.10 | Error | a concrete `Expression`'s `element` names an abstract `Term` or `Expression` (OCL `self.isAbstract = false implies self.element->forall(...)`): one finding per distinct id |
//! | SACM-CITE-002 | SACM23-VAL-019 | §11.8 | Warning | an Assertion declared `asCited` is not a citation (`isCitation` not true), has no `citedElement`, or cites an element that is no Assertion: one finding each |
//! | SACM-PKG-001 | SACM23-VAL-013 | §10.5, §10.6, §11.5, §11.6, §12.4 or §12.5 | Error | a child in the family role of a package interface or binding is no citation (`isCitation` true and a `citedElement`), an `AssertedRelationship` in an `ArgumentPackageBinding` and an `ArtifactAssetRelationship` in an `ArtifactPackageBinding` excepted |
//! | SACM-PKG-001 | SACM23-VAL-013 | §10.5, §11.6 or §12.5 | Warning | an interface's citation cites an element of the document that is not of the kind the clause names, or that is not inside the package the interface implements: one finding each |
//! | SACM-PKG-001 | SACM23-VAL-015 | §9.3 | Error | a child of an `AssuranceCasePackageInterface` is not exactly the interface class of its role (the §9.3 OCL) |
//! | SACM-MULT-001 | SACM23-VAL-014 | §9.3, §10.5, §11.6 or §12.5 | Error | a package interface has no `implements` (`implements:<Package>[1]`) |
//! | SACM-MULT-001 | SACM23-VAL-014 | §9.4, §10.6, §11.5 or §12.4 | Error | a package binding has fewer than two distinct participants, ids and `href`s counted (`participantPackage:<Package>[2..*]`) |
//! | SACM-PKG-001 | SACM23-VAL-014 | §9.4, §10.6, §11.5 or §12.4 | Warning | a binding names a binding of its own kind as a participant (D-BIND-PART) |
//! | SACM-PKG-002 | SACM23-VAL-023 | §10.5, §11.6 or §12.5 | Warning | an interface whose `isCitation` is not true does not reside inside the package its `implements` names |
//!
//! [`validate`]'s docs say what each rule counts. The [diagnostics catalog][catalog]
//! gives, for each code, the elements a finding affects and how to fix it.
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
//!
//! [catalog]: https://github.com/lasrod/libSACM/blob/main/docs/sacm/diagnostics.md

mod constraints;
mod descriptions;
mod expressions;
mod gid;
mod homogeneity;
mod ids;
mod metadata;
pub(crate) use metadata::{Found, tokens, walk_document, walk_fragments};
mod modularity;
mod multi_lang;
mod names;
mod references;
mod relationships;
mod repeats;
mod terminology;

use crate::diagnostic::Diagnostic;
use crate::model::Document;

/// A rule: pushes its findings about the document.
type Rule = fn(&Document, &mut Vec<Diagnostic>);

/// Every rule, run in turn by [`validate`].
const RULES: &[Rule] = &[
    names::missing_name,
    multi_lang::multi_lang_strings,
    expressions::expression_lang_strings,
    ids::invalid_id,
    gid::duplicate_gid,
    descriptions::repeated_description,
    homogeneity::argument_package_homogeneity,
    modularity::interface_and_binding_content,
    modularity::assurance_case_interface_content,
    modularity::implements_and_participants,
    modularity::interface_placement,
    references::dangling_references,
    references::wrongly_typed_references,
    references::citation,
    references::as_cited,
    references::abstract_form,
    constraints::implementation_constraints,
    relationships::relationship_ends,
    relationships::artifact_reference_kinds,
    repeats::repeated_references,
    terminology::missing_values,
    terminology::concrete_expressions,
];

/// Checks `document` against the SACM rules this release knows and returns
/// the findings, sorted by [`Diagnostic`]'s order.
///
/// - **Read-only.** The document is not changed. The [module docs](self)
///   list what is checked.
/// - **An empty result** means no rule found anything, not that the document
///   conforms in full.
/// - **Rules are added.** A later minor release may add a rule. That is not a
///   breaking change, and it never changes what a strict
///   [`xmi::write`](crate::xmi::write) writes, which checks schema validity
///   only (ADR 0035 D4).
///
/// # What the rules count
///
/// The [diagnostics catalog][catalog] gives, for each code, the elements a
/// finding affects and how to fix it.
///
/// - **Names.** Every `ModelElement`
///   ([`AbstractClass::ModelElement`](crate::model::AbstractClass::ModelElement))
///   needs one; the four `UtilityElement` kinds have none. An empty name
///   (`<name/>`) is a name.
/// - **Languages.** The `lang`s of a `content`, `key` or `location` are
///   compared exactly: two absent ones are equal, an absent one and `""`
///   differ, and so do `en` and `EN`.
/// - **Absent texts.** An absent `content` or `location` gives no finding; a
///   `TaggedValue` must have a `key`.
/// - **Ids.** [`xmi::read`](crate::xmi::read) reads an id that is no `NCName`,
///   such as `1c`, `a:b` or `a b`, as written, so that nothing is lost.
///   [`edit::apply`](crate::edit::apply) never creates one.
/// - **Ids of names and texts.** The `xmi:id` of a name
///   ([`SacmElement::name_id`](crate::model::SacmElement::name_id)) or a text
///   ([`SacmElement::content_id`](crate::model::SacmElement::content_id) and
///   the like) is checked too; a finding affects the element that owns it.
/// - **Descriptions.** Two or more are SACM-MULT-001, a Warning, and every one
///   is kept. Notes, implementation constraints and tagged values are not
///   counted.
/// - **Why a Warning.** The text of §8.6 allows one description
///   (`description:Description[0..1]`), the model ptc/22-03-13 any number. The
///   crate holds them all, so such files round-trip, and reports the text's
///   bound (D-DESC-MULT).
/// - **`gid`.** Compared exactly across the whole document, on every kind. An
///   absent or empty (`""`) gid is not compared (D-GID-EMPTY). A gid is not
///   an `xmi:id`: a gid equal to an id is no finding.
/// - **Nested `ArgumentPackage`s.** Only a plain `ArgumentPackage` is checked
///   as an owner, for its own children. An interface or binding child neither
///   triggers the rule nor is reported, and `UtilityElement`s are not counted.
/// - **References.** A reference is held as the id it names (ADR 0026). A
///   reference to any element resolves, the element itself included. The
///   `xmi:id` of a `name`, `content`, `key`, `location` or `value` names none.
/// - **One finding per distinct id** of each end of each element. An id the
///   declared-type rule reports (SACM23-VAL-004) is not reported again by the
///   `ArtifactReference` rule (SACM23-VAL-016).
/// - **A reference that names no element** is only SACM-REF-001. The rules
///   about the element an `abstractForm`, or the `citedElement` of an
///   `asCited` Assertion, names do not report it.
/// - **Repeated ids.** A multi-valued end holds its ids as read, repeats
///   included (ADR 0027). A repeat is reported whatever else applies to the
///   id, and a strict save still writes it.
/// - **Preserved content.** An id of an element held as preserved content
///   (ADR 0033) is SACM-REF-003, never SACM-REF-001 or SACM-REF-002.
///   References inside preserved content are not checked.
/// - **Citations.** A citation has `isCitation` true and a `citedElement`, an
///   id or an `href`. `isCitation` true alone is no finding. A `citedElement`
///   without it is SACM-CITE-001, whether or not the cited id resolves.
/// - **Cross-document references.** An `href` (SACM23-XMI-027) counts as a
///   value of its end. Its target is never checked: an `href` is never
///   SACM-REF-001, SACM-REF-002 or SACM-REF-003 (ADR 0040 D6).
///
/// # Declared types
///
/// SACM-REF-002 reports an id that names an element its end's declared type
/// (ptc/22-03-13, on which the text agrees) does not admit. The catalog
/// gives the types of the ends not listed here.
///
/// | End | Declared type |
/// |---|---|
/// | `citedElement`, `abstractForm` | `SACMElement` |
/// | `metaClaim` | `Claim` |
/// | `reasoning` | `ArgumentReasoning` |
/// | `source`, `target` | `ArgumentAsset` |
/// | `structure` | `ArgumentPackage` |
/// | `referencedArtifactElement` | `ArtifactElement` (every kind but the `UtilityElement`s) |
/// | an `ArgumentGroup`'s `argumentElement` | `ArgumentationElement` |
/// | an `ArtifactGroup`'s `artifactElement` | `ArtifactElement` |
///
/// # Examples
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
///
/// Two values of one `content` in the same language are one finding:
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::edit::{self, CreateArgumentPackage, CreateNote};
/// use sacm::model::{Document, LangString, MultiLangString};
/// use sacm::validation;
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// // A Note whose content says two things in English.
/// let content: MultiLangString = [
///     LangString::new("Reviewed").with_lang("en"),
///     LangString::new("Checked").with_lang("en"),
/// ]
/// .into_iter()
/// .collect();
/// edit::apply(&mut doc, CreateNote::new("p", content).with_id("n1"))?;
///
/// let found = validation::validate(&doc);
/// assert_eq!(found.len(), 1);
/// assert_eq!(found[0].code(), Code::MULTIPLICITY_VIOLATION);
/// assert_eq!(found[0].affected()[0].as_str(), "n1");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
///
/// An `abstractForm` that names a concrete element is a Warning, affecting
/// the element, then the element it names:
///
/// ```
/// use sacm::diagnostic::{Code, Severity};
/// use sacm::validation;
/// use sacm::xmi::{self, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name content="Arguments"/>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" abstractForm="c2">
///     <name content="The brakes are safe"/>
///   </argumentElement>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2">
///     <name content="The pump is safe"/>
///   </argumentElement>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(input, &ReadOptions::default())?.into_document();
/// // c2 is not abstract, so c1 should not conform to it.
/// let found = validation::validate(&document);
/// assert_eq!(found.len(), 1);
/// assert_eq!((found[0].code(), found[0].severity()), (Code::ABSTRACT_FORM_INVALID, Severity::Warning));
/// let affected: Vec<&str> = found[0].affected().iter().map(|id| id.as_str()).collect();
/// assert_eq!(affected, ["c1", "c2"]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [catalog]: https://github.com/lasrod/libSACM/blob/main/docs/sacm/diagnostics.md
#[must_use]
pub fn validate(document: &Document) -> Vec<Diagnostic> {
    run(document, RULES)
}

/// The rules of the schema check a strict [`xmi::write`](crate::xmi::write)
/// runs (LIB-COMPAT-007, ADR 0035 D4): an `xmi:id` that is no `NCName`
/// (XML Schema 1.0 Part 2 §3.3.8), and a reference to no `xmi:id` the output
/// writes (Part 1 §3.3.4, XMI 2.5.1 rule 4i), of an element or in its XMI
/// metadata (SACM23-XMI-019, ADR 0042).
const SCHEMA_RULES: &[Rule] = &[
    ids::invalid_id,
    references::unwritten_references,
    metadata::unwritten,
];

/// The Errors that make the XMI output of `document` schema-invalid, sorted
/// by [`Diagnostic`]'s order: one SACM-ID-002 per `xmi:id` that is no
/// `NCName`, and one SACM-REF-001 per distinct reference id of an end that
/// names no `xmi:id` the output writes, of elements and of XMI metadata. It is no part of [`validate`], which
/// it never runs (ADR 0035 D4).
pub(crate) fn schema_errors(document: &Document) -> Vec<Diagnostic> {
    run(document, SCHEMA_RULES)
}

/// The findings of `rules` about `document`, sorted.
fn run(document: &Document, rules: &[Rule]) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    for rule in rules {
        rule(document, &mut found);
    }
    found.sort();
    found
}
