//! A class outside SACM 2.3 that an element was read as an instance of,
//! because the application declared it a subclass of the element's kind
//! (LIB-COMPAT-017, ADR 0044 D3).

/// A class outside SACM 2.3 that a tolerant read read an element as an
/// instance of, because the application declared it a subclass of the
/// element's kind
/// ([`ReadOptions::with_subclass`](crate::xmi::ReadOptions::with_subclass),
/// LIB-COMPAT-017).
///
/// [`SacmElement::subclass`](super::SacmElement::subclass) gives it. It is
/// preserved content of the element: a tolerant write writes it back as the
/// element's `xmi:type`, and a strict write refuses the element.
///
/// ```
/// use sacm::model::ElementKind;
/// use sacm::xmi::{self, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:p="urn:example:profile"
///     xmi:id="arguments">
///   <argumentElement xmi:type="p:Step" xmi:id="i1"/>
/// </sacm:ArgumentPackage>"#;
/// let options = ReadOptions::default()
///     .with_subclass("urn:example:profile", "Step", ElementKind::AssertedInference);
/// let document = xmi::read(input, &options)?.into_document();
/// let class = document.get("i1").and_then(|element| element.subclass());
/// assert_eq!(
///     class.map(|class| (class.namespace(), class.name())),
///     Some(("urn:example:profile", "Step"))
/// );
/// # Ok::<(), xmi::ReadError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Subclass<'a> {
    namespace: &'a str,
    name: &'a str,
}

impl<'a> Subclass<'a> {
    /// The class `name` of the namespace `namespace`.
    pub(crate) fn new(namespace: &'a str, name: &'a str) -> Self {
        Self { namespace, name }
    }

    /// The class's namespace URI.
    #[must_use]
    pub fn namespace(self) -> &'a str {
        self.namespace
    }

    /// The class's name: the local part of its type.
    #[must_use]
    pub fn name(self) -> &'a str {
        self.name
    }
}
