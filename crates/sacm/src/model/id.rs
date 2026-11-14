//! Element identity.

use std::borrow::Borrow;
use std::fmt;

/// The identifier of an element, its `xmi:id` or an id generated for it,
/// and also the type of a name's `xmi:id`
/// ([`SacmElement::name_id`](crate::model::SacmElement::name_id)).
///
/// An `ElementId` keeps its value exactly as given. It is not trimmed, case
/// folded or otherwise normalised. An id read from a file is its `xsd:ID`
/// value: [`xmi::read`](crate::xmi::read) trims XML white space around an
/// `NCName` and otherwise keeps the id as written, and
/// [`xmi::write`](crate::xmi::write) writes it back as its `xsd:ID` value.
/// Equality, ordering and hashing are those of the
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

impl From<&ElementId> for ElementId {
    /// A copy of the id, so that functions taking `impl Into<ElementId>`
    /// accept an `&ElementId` too.
    ///
    /// ```
    /// use sacm::model::ElementId;
    ///
    /// let id = ElementId::from("arguments");
    /// assert_eq!(ElementId::from(&id), id);
    /// ```
    fn from(value: &ElementId) -> Self {
        value.clone()
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

/// Whether `value` is an XML `NCName`: the XML 1.0 (fifth edition) `Name`
/// production (§2.3, productions [4], [4a] and [5]) without `:`, as
/// Namespaces in XML 1.0 defines it. An `xmi:id` is an `xsd:ID`, which is an
/// `NCName` (XMI 2.5.1 §7.6.1).
pub(crate) fn is_ncname(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if is_name_start_char(first) => chars.all(is_name_char),
        _ => false,
    }
}

/// XML 1.0 production [4] `NameStartChar`, without `:`.
fn is_name_start_char(c: char) -> bool {
    matches!(c,
        'A'..='Z'
        | '_'
        | 'a'..='z'
        | '\u{C0}'..='\u{D6}'
        | '\u{D8}'..='\u{F6}'
        | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}'
        | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}'
    )
}

/// XML 1.0 production [4a] `NameChar`, without `:`.
fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || matches!(c,
            '-'
            | '.'
            | '0'..='9'
            | '\u{B7}'
            | '\u{300}'..='\u{36F}'
            | '\u{203F}'..='\u{2040}'
        )
}

#[cfg(test)]
mod tests {
    use super::is_ncname;

    #[test]
    fn ncname_boundaries() {
        for good in [
            "a",
            "_",
            "Z9",
            "a-b.c",
            "x\u{B7}y",
            "\u{C0}",
            "\u{D8}",
            "\u{F8}",
            "\u{2FF}",
            "\u{370}",
            "\u{37D}",
            "\u{37F}",
            "\u{1FFF}",
            "\u{200C}",
            "\u{200D}",
            "\u{2070}",
            "\u{218F}",
            "\u{2C00}",
            "\u{2FEF}",
            "\u{3001}",
            "\u{D7FF}",
            "\u{F900}",
            "\u{FDCF}",
            "\u{FDF0}",
            "\u{FFFD}",
            "\u{10000}",
            "\u{EFFFF}",
            "a\u{300}",
            "a\u{36F}",
            "a\u{203F}",
            "a\u{2040}",
            "a0",
            "a9",
        ] {
            assert!(is_ncname(good), "{good:?} is an NCName");
        }
        for bad in [
            "",
            ":",
            "a:b",
            "0",
            "-",
            ".",
            "\u{B7}",
            "\u{D7}",
            "\u{F7}",
            "\u{300}",
            "\u{37E}",
            "\u{2000}",
            "\u{200B}",
            "\u{200E}",
            "\u{2190}",
            "\u{2FF0}",
            "\u{3000}",
            "\u{FDD0}",
            "\u{FFFE}",
            "\u{F0000}",
            "a b",
            "a/b",
            "a\u{2041}",
            "a\u{203E}",
        ] {
            assert!(!is_ncname(bad), "{bad:?} is not an NCName");
        }
    }
}
