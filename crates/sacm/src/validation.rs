//! Validation: checking a document against SACM's rules.
//!
//! A document read from a file can break SACM's rules and still loads, so
//! that it can be inspected and fixed. [`validate`] reports what is wrong as
//! [`Diagnostic`]s. It never changes a document and never refuses one.
//!
//! # Checked in this release
//!
//! | Code | Row | Clause | Severity | Finding |
//! |---|---|---|---|---|
//! | SACM-MULT-001 | SACM23-VAL-018 | §8.6 | Error | a `ModelElement` has no name (`name:LangString[1]`) |
//!
//! The result is sorted by [`Diagnostic`]'s order. Its diagnostics have no
//! [`location`](Diagnostic::location), because a [`Document`] records no
//! source positions.
//!
//! Later releases add rules, so a document with no findings today may have
//! findings later. A code's meaning never changes.
//!
//! To report reading and validation together, extend the
//! [`ReadOutput::diagnostics`](crate::xmi::ReadOutput::diagnostics) with the
//! result and sort it again.
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
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod names;

use crate::diagnostic::Diagnostic;
use crate::model::Document;

/// A rule: pushes its findings about the document.
type Rule = fn(&Document, &mut Vec<Diagnostic>);

/// Every rule, run in turn by [`validate`].
const RULES: &[Rule] = &[names::missing_name];

/// Checks `document` against every rule and returns the findings, sorted by
/// [`Diagnostic`]'s order.
///
/// The document is not changed. See the [module docs](self) for what is
/// checked.
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
