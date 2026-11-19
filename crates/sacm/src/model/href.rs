//! Cross-document references (SACM23-XMI-027, ADR 0040).

use std::fmt;

/// A reference to an element of another document, held as the `href` that
/// names it (XMI 2.5.1 §7.10.2 `URI#id_value`): a document part and a
/// fragment, exactly as read (white space around it trimmed), never resolved
/// (SACM23-XMI-027, ADR 0040).
/// [`SacmElement::hrefs`](crate::model::SacmElement::hrefs) gives an end's.
///
/// It holds one `#`, with a document part before it and a fragment after it,
/// neither empty, and no XML white space, `(` or `)`. Nothing is decoded or
/// normalised: `a%20b.xmi#c` stays as written. Equality, ordering and hashing
/// are those of the string. No file is opened: [`document`](Self::document)
/// and [`fragment`](Self::fragment) let an application load the other
/// document and find the element itself.
///
/// # Examples
///
/// ```
/// use sacm::model::ReferenceEnd;
/// use sacm::xmi::{self, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackageBinding xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="binding">
///   <name content="Braking supports the vehicle"/>
///   <participantPackage href="braking.sacm.xmi#braking_api"/>
/// </sacm:ArgumentPackageBinding>"#;
/// let doc = xmi::read(input, &ReadOptions::default())?.into_document();
/// let binding = doc.get("binding").ok_or("not read")?;
/// let href = &binding.hrefs(ReferenceEnd::ParticipantPackage)[0];
/// assert_eq!(href.as_str(), "braking.sacm.xmi#braking_api");
/// assert_eq!((href.document(), href.fragment()), ("braking.sacm.xmi", "braking_api"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Href(String);

impl Href {
    /// The `href` `value`, which the reader has checked holds one `#` with a
    /// non-empty part on each side, and no XML white space, `(` or `)`.
    pub(crate) fn new_unchecked(value: &str) -> Self {
        debug_assert!(
            value.split_once('#').is_some_and(|(document, fragment)| {
                !document.is_empty() && !fragment.is_empty() && !fragment.contains('#')
            }),
            "an href holds a document part and a fragment"
        );
        Self(value.to_owned())
    }

    /// The whole value, such as `braking.sacm.xmi#braking_api`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The part before `#`: the document, as written (not resolved or
    /// normalised), such as `braking.sacm.xmi`.
    #[must_use]
    pub fn document(&self) -> &str {
        self.0.split_once('#').map_or("", |(document, _)| document)
    }

    /// The part after `#`: the id of the element in that document, as
    /// written, such as `braking_api`.
    #[must_use]
    pub fn fragment(&self) -> &str {
        self.0.split_once('#').map_or("", |(_, fragment)| fragment)
    }
}

impl fmt::Display for Href {
    /// The whole value, as [`as_str`](Self::as_str); width and alignment
    /// apply as to a [`str`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl AsRef<str> for Href {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::Href;

    #[test]
    fn the_parts_split_at_the_one_hash() {
        let href = Href::new_unchecked("http://example.org/a/b.xmi#c_1");
        assert_eq!(href.document(), "http://example.org/a/b.xmi");
        assert_eq!(href.fragment(), "c_1");
        assert_eq!(format!("{href:>34}"), format!("{:>34}", href.as_str()));
    }
}
