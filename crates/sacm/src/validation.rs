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
//! | SACM-ID-002 | SACM23-VAL-020 | XMI 2.5.1 §7.6.1 | Error | an element's id is not an XML `NCName` (an `xmi:id` is an `xsd:ID`) |
//!
//! A `ModelElement`
//! ([`AbstractClass::ModelElement`](crate::model::AbstractClass::ModelElement))
//! is any named, identifiable element: every package, claim, asserted
//! relationship, artifact and term. The four `UtilityElement` kinds
//! (`Description`, `ImplementationConstraint`, `Note`, `TaggedValue`) have no
//! name and are not checked. An empty name (`<name/>`) is a name. To fix a
//! finding, give the element a `<name lang="…" content="…"/>` child.
//!
//! Every element's id is checked. An `NCName` starts with a letter or `_`
//! and continues with letters, digits, `-`, `.` or `_` (XML 1.0 Fifth
//! Edition and Namespaces in XML Third Edition give the full character
//! ranges); it has no spaces and no `:`. [`xmi::read`](crate::xmi::read)
//! reads an id such as `1c`, `a:b` or `a b` as written, so that nothing is
//! lost, and refuses only an empty one, except an `NCName` with XML white
//! space around it, such as `" c1 "`, which is `Unsupported` until a later
//! release; [`edit::apply`](crate::edit::apply) never creates one. To fix a finding, give the element an id that is an
//! `NCName`.
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
//! reports an `xsi:type` (SACM-XMI-010) and an `xmi:version` (SACM-XMI-011);
//! it refuses what it cannot represent.
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

mod ids;
mod names;

use crate::diagnostic::Diagnostic;
use crate::model::Document;

/// A rule: pushes its findings about the document.
type Rule = fn(&Document, &mut Vec<Diagnostic>);

/// Every rule, run in turn by [`validate`].
const RULES: &[Rule] = &[names::missing_name, ids::invalid_id];

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
