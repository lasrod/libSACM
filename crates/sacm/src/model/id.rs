//! Element identity.

use std::borrow::Borrow;
use std::fmt;

/// The identifier of an element: its `xmi:id`.
///
/// An `ElementId` keeps its value exactly as given. It is not trimmed, case
/// folded or otherwise normalised, so an id read from a file is written back
/// byte for byte. Equality, ordering and hashing are those of the
/// string, and an `ElementId` borrows as a [`str`], so maps and sets keyed by
/// `ElementId` can be searched with a `&str`.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeSet;
/// use sacm::model::ElementId;
///
/// let id = ElementId::new("claim_top");
/// assert_eq!(id.as_str(), "claim_top");
/// assert_eq!(id.to_string(), "claim_top");
///
/// let ids: BTreeSet<ElementId> = ["b", "a"].into_iter().map(ElementId::from).collect();
/// assert!(ids.contains("a"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementId(String);

impl ElementId {
    /// An id with exactly the given value.
    ///
    /// The value is kept as it is. In particular, `new` accepts the empty
    /// string; whether an empty id is acceptable is up to the operation that
    /// receives it.
    ///
    /// ```
    /// use sacm::model::ElementId;
    ///
    /// assert_eq!(ElementId::new("  padded  ").as_str(), "  padded  ");
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The id's value.
    ///
    /// ```
    /// use sacm::model::ElementId;
    ///
    /// assert_eq!(ElementId::from("_acp.1").as_str(), "_acp.1");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ElementId {
    /// An id with exactly the given value; see [`ElementId::new`].
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ElementId {
    /// An id with exactly the given value; see [`ElementId::new`].
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for ElementId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ElementId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ElementId {
    /// Writes the id's value, unquoted and unchanged.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}
