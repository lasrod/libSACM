//! Cross-document references (SACM23-XMI-027, SACM23-XMI-029, ADR 0040).

use std::fmt;

use super::ElementKind;

/// A reference to an element of another document, or an `XPointer`, held as
/// the `href` that names it (XMI 2.5.1 §7.10.2 `URI#id_value`, rule 2n): an
/// optional type, then a document part and a fragment, as read (white space
/// around it trimmed, and a type written as `sacm:<Class>` and one space),
/// never resolved (SACM23-XMI-027, SACM23-XMI-029, ADR 0040).
/// [`SacmElement::hrefs`](crate::model::SacmElement::hrefs) gives an end's,
/// [`LangString::expression_href`](crate::model::LangString::expression_href)
/// an `ExpressionLangString`'s, and a [`Proxy`] holds one. [`Href::new`]
/// makes one for an edit.
///
/// Every character of it is an XML 1.0 `Char` (XML 1.0 §2.2). Its URI
/// reference ([`uri`](Self::uri)) holds one `#`, with a fragment after it,
/// and no XML white space. The fragment is the id of an element of
/// the other document, or an `XPointer` ([`is_xpointer`](Self::is_xpointer));
/// the document part before the `#` is empty only for an `XPointer` into the
/// same document. A type may stand before the URI reference (XMI 2.5.1 rule
/// 2n), held as `sacm:<Class>` and one space: [`kind`](Self::kind) gives the
/// class. The URI reference is never decoded or normalised: `a%20b.xmi#c`
/// stays as written.
/// Equality, ordering and hashing are those of the string. No file is
/// opened: [`document`](Self::document) and [`fragment`](Self::fragment) let
/// an application load the other document and find the element itself.
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

/// The prefix of an href's type, as it is held: the type is `sacm:<Class>`.
const TYPE_PREFIX: &str = "sacm:";

/// XML white space (XML 1.0 production 3).
fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// Whether `c` is an XML 1.0 `Char` (§2.2): what an XMI document can hold.
/// A Rust `char` is never a surrogate.
pub(crate) fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | ' '..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/// The concrete class `name` names, if it names one.
fn concrete_class(name: &str) -> Option<ElementKind> {
    ElementKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.name() == name)
}

impl Href {
    /// The cross-document reference `value`, as [`as_str`](Self::as_str)
    /// gives one: an optional type `sacm:<Class>` and one space, then a URI
    /// reference `document#fragment`, the document part empty only for an
    /// `XPointer` (ADR 0040 amendment 1).
    ///
    /// Nothing is trimmed, decoded, normalised or resolved, and no file is
    /// opened: the element it names need not exist (ADR 0040 D6, D9).
    ///
    /// ```
    /// use sacm::model::{ElementKind, Href, HrefError};
    ///
    /// let plain = Href::new("braking.sacm.xmi#braking_api")?;
    /// assert_eq!(plain.as_str(), "braking.sacm.xmi#braking_api");
    /// let typed = Href::new("sacm:Claim braking.sacm.xmi#c1")?;
    /// assert_eq!(typed.kind(), Some(ElementKind::Claim));
    /// // An element of the same document is named by its id, not by an href.
    /// assert_eq!(Href::new("#c1"), Err(HrefError::SameDocument));
    /// # Ok::<(), HrefError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`HrefError`], the first problem in the order of its variants.
    pub fn new(value: &str) -> Result<Self, HrefError> {
        if !value.chars().all(is_xml_char) {
            return Err(HrefError::Character);
        }
        // The one space after a type is the only white space allowed: it
        // stands between two parts that are not empty, and what is before it
        // holds no `#`, or the space would be inside the URI reference.
        let mut spaces = value.chars().filter(|c| is_xml_space(*c));
        let uri = match (spaces.next(), spaces.next()) {
            (None, _) => value,
            (Some(' '), None) => match value.split_once(' ') {
                Some((class, uri))
                    if !class.is_empty() && !uri.is_empty() && !class.contains('#') =>
                {
                    let known = class
                        .strip_prefix(TYPE_PREFIX)
                        .is_some_and(|name| concrete_class(name).is_some());
                    if !known {
                        return Err(HrefError::Type);
                    }
                    uri
                }
                _ => return Err(HrefError::WhiteSpace),
            },
            _ => return Err(HrefError::WhiteSpace),
        };
        let Some((document, fragment)) = uri.split_once('#') else {
            return Err(HrefError::NoFragment);
        };
        if fragment.is_empty() {
            return Err(HrefError::NoFragment);
        }
        if fragment.contains('#') {
            return Err(HrefError::HashInFragment);
        }
        if document.is_empty() && !fragment.contains(['(', ')']) {
            return Err(HrefError::SameDocument);
        }
        Ok(Self(value.to_owned()))
    }

    /// The `href` of the URI reference `uri` with the type `kind`, if it has
    /// one: what the reader holds, having checked that `uri` is
    /// `document#fragment` with no XML white space and one `#`, and is no
    /// `#id`.
    pub(crate) fn from_parts(kind: Option<ElementKind>, uri: &str) -> Self {
        let value = match kind {
            Some(kind) => format!("{TYPE_PREFIX}{} {uri}", kind.name()),
            None => uri.to_owned(),
        };
        debug_assert!(
            Self::new(&value).is_ok(),
            "the reader holds only what Href::new accepts"
        );
        Self(value)
    }

    /// The whole value, such as `braking.sacm.xmi#braking_api`, or, with a
    /// type, `sacm:Claim braking.sacm.xmi#c1`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The concrete class the href's type names (XMI 2.5.1 rule 2n: "A URI
    /// Reference, optionally preceded by the type of the object being
    /// referenced"), or `None` when it has no type. The class is what the
    /// file says, never checked against the other document.
    ///
    /// ```
    /// use sacm::model::{ElementKind, Href};
    ///
    /// let typed = Href::new("sacm:ArgumentPackageInterface braking.sacm.xmi#api")?;
    /// assert_eq!(typed.kind(), Some(ElementKind::ArgumentPackageInterface));
    /// assert_eq!(Href::new("braking.sacm.xmi#api")?.kind(), None);
    /// # Ok::<(), sacm::model::HrefError>(())
    /// ```
    #[must_use]
    pub fn kind(&self) -> Option<ElementKind> {
        let (class, _) = self.0.split_once(' ')?;
        concrete_class(class.strip_prefix(TYPE_PREFIX)?)
    }

    /// The URI reference, after the type: `document#fragment`.
    ///
    /// ```
    /// use sacm::model::Href;
    ///
    /// let typed = Href::new("sacm:Claim braking.sacm.xmi#c1")?;
    /// assert_eq!(typed.uri(), "braking.sacm.xmi#c1");
    /// assert_eq!((typed.document(), typed.fragment()), ("braking.sacm.xmi", "c1"));
    /// # Ok::<(), sacm::model::HrefError>(())
    /// ```
    #[must_use]
    pub fn uri(&self) -> &str {
        self.0.split_once(' ').map_or(&self.0, |(_, uri)| uri)
    }

    /// The part of the URI reference before `#`: the document, as written
    /// (not resolved or normalised), such as `braking.sacm.xmi`. Empty only
    /// for an `XPointer` into the same document.
    #[must_use]
    pub fn document(&self) -> &str {
        self.uri()
            .split_once('#')
            .map_or("", |(document, _)| document)
    }

    /// The part after `#`: the id of the element in that document, or an
    /// `XPointer` ([`is_xpointer`](Self::is_xpointer)), as written, such as
    /// `braking_api`.
    #[must_use]
    pub fn fragment(&self) -> &str {
        self.uri()
            .split_once('#')
            .map_or("", |(_, fragment)| fragment)
    }

    /// Whether the fragment is an `XPointer` (it holds `(` or `)`), not the id
    /// of an element. The library holds an `XPointer` as written and never
    /// evaluates it.
    ///
    /// ```
    /// use sacm::model::Href;
    ///
    /// let pointer = Href::new("#xpointer(id('c2'))")?;
    /// assert!(pointer.is_xpointer());
    /// assert_eq!((pointer.document(), pointer.fragment()), ("", "xpointer(id('c2'))"));
    /// assert!(!Href::new("o.xmi#c2")?.is_xpointer());
    /// # Ok::<(), sacm::model::HrefError>(())
    /// ```
    #[must_use]
    pub fn is_xpointer(&self) -> bool {
        self.fragment().contains(['(', ')'])
    }
}

/// The JSON of an [`Href`]: the string [`Href::as_str`] gives (ADR 0040
/// amendment 1, A7).
#[cfg(feature = "serde")]
impl serde::Serialize for Href {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Reads a JSON string, checked by [`Href::new`]; the error is the
/// [`HrefError`]'s `Display`.
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Href {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Href::new(&value).map_err(serde::de::Error::custom)
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

/// Why [`Href::new`] refused a value. The variants are in the order of the
/// checks: a value with two problems gives the earlier one.
///
/// ```
/// use sacm::model::{Href, HrefError};
///
/// assert_eq!(Href::new("o.xmi"), Err(HrefError::NoFragment));
/// assert_eq!(Href::new("x:Claim o.xmi"), Err(HrefError::Type));
/// assert_eq!(
///     HrefError::NoFragment.to_string(),
///     "the href has no \"#\" followed by a fragment"
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HrefError {
    /// The value holds a character that is not an XML 1.0 `Char` (XML 1.0
    /// §2.2), which no XMI document can hold.
    Character,
    /// The value holds XML white space other than the one space after its
    /// type.
    WhiteSpace,
    /// What stands before the one space is not `sacm:` followed by the name
    /// of a concrete SACM 2.3 class.
    Type,
    /// The URI reference has no `#`, or nothing after it.
    NoFragment,
    /// The fragment holds a second `#`.
    HashInFragment,
    /// The URI reference is `#id`: an element of the same document, which is
    /// named by its id.
    SameDocument,
}

impl fmt::Display for HrefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Character => "the href holds a character that XML 1.0 cannot represent",
            Self::WhiteSpace => {
                "the href holds XML white space other than the one space after its type"
            }
            Self::Type => {
                "the type before the href's URI reference is not \"sacm:\" followed by the name of a concrete SACM 2.3 class"
            }
            Self::NoFragment => "the href has no \"#\" followed by a fragment",
            Self::HashInFragment => "the href's fragment holds a \"#\"",
            Self::SameDocument => {
                "the href names an element of the same document by its id: use the id instead"
            }
        })
    }
}

impl std::error::Error for HrefError {}

/// A proxy (XMI 2.5.1 §7.10.1): a child, in one of an element's composite
/// roles, that is held in another document and named by an href; never
/// resolved (SACM23-XMI-029, ADR 0040 amendment 1).
/// [`SacmElement::proxies`](crate::model::SacmElement::proxies) gives an
/// element's.
///
/// A proxy is no element of this document: it has no id, it is no child,
/// and no validation rule counts it. Proxies order by role, then by href.
///
/// ```
/// use sacm::xmi::{self, ReadOptions};
///
/// let input = br#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
///   <name content="Case"/>
///   <argumentPackage href="arguments.sacm.xmi#args"/>
/// </sacm:AssuranceCasePackage>"#;
/// let doc = xmi::read(input, &ReadOptions::default())?.into_document();
/// let case = doc.get("case").ok_or("not read")?;
/// let [proxy] = case.proxies() else { return Err("not one proxy".into()) };
/// assert_eq!(proxy.role(), "argumentPackage");
/// assert_eq!(proxy.href().document(), "arguments.sacm.xmi");
/// assert_eq!(case.children().count(), 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Proxy {
    role: &'static str,
    href: Href,
}

impl Proxy {
    /// The proxy in `role` that `href` names.
    pub(crate) fn new(role: &'static str, href: Href) -> Self {
        Self { role, href }
    }

    /// The composite role, by its XMI name, such as `argumentPackage`.
    #[must_use]
    pub fn role(&self) -> &'static str {
        self.role
    }

    /// The href that names the child; its [`kind`](Href::kind) is the
    /// child's class, if the file gives one.
    #[must_use]
    pub fn href(&self) -> &Href {
        &self.href
    }
}

#[cfg(test)]
mod tests {
    use super::Href;

    #[test]
    fn the_parts_split_at_the_one_hash() {
        let href = Href::from_parts(None, "http://example.org/a/b.xmi#c_1");
        assert_eq!(href.document(), "http://example.org/a/b.xmi");
        assert_eq!(href.fragment(), "c_1");
        assert_eq!(format!("{href:>34}"), format!("{:>34}", href.as_str()));
    }
}
