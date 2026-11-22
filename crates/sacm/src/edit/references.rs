//! The operations that edit the reference ends of an existing element, and
//! the check of referenced ids that they share with the creates
//! (LIB-EDIT-009, ADR 0027, ADR 0028, ADR 0029).

use std::collections::BTreeSet;
use std::slice;

use super::EditError;
use super::plan::{Plan, Value};
use super::set::{changing, edited};
use crate::model::{Document, ElementId, ElementKind, Href, LangString, ReferenceEnd, SacmElement};

/// Replaces the ids one reference end of an element holds (SACM 2.3 §8.2,
/// §11.2, §11.9, §11.10, §11.12, §11.13), in the order given, repeats
/// included; [`clear`](Self::clear) empties the end. The end then holds
/// exactly these ids and the cross-document references given with
/// [`with_hrefs`](Self::with_hrefs), none unless some are given
/// ([`SacmElement::hrefs`](crate::model::SacmElement::hrefs); ADR 0040 D7,
/// amendment 1).
///
/// The ids must fit the end's bounds: `source` needs at least one, `target`
/// and an interface's `implements` exactly one, a binding's
/// `participantPackage` at least two distinct ids (ADR 0039 D3), and the
/// other single-valued ends (`citedElement`, `abstractForm`, `reasoning`,
/// `structure`, `origin`) hold at most one. An href counts as a value
/// beside the ids: a lower bound counts the distinct ids plus the distinct
/// hrefs, and an upper bound every value (ADR 0040 D5). Every id the
/// end does not already hold must name an element of the document, of a
/// kind its end admits, so an edit never makes a dangling or wrongly typed
/// reference; an id the end holds is kept as it is, even if it is dangling
/// or wrongly typed, so that the end can be repaired one id at a time (ADR
/// 0029). Apply it with [`edit::apply`](super::apply). It refuses, in
/// order:
/// 1. an unknown element, as [`EditError::ElementNotFound`];
/// 2. the derived [`ReferenceEnd::Expression`], whose ids the element's
///    name and texts hold, as [`EditError::DerivedEnd`] (SACM23-BASE-009);
/// 3. a kind that does not own the end, as
///    [`EditError::FeatureNotApplicable`];
/// 4. fewer values than the end needs, as [`EditError::TooFewReferences`];
/// 5. more values than it allows, as [`EditError::TooManyReferences`];
/// 6. then, over the ids the end does not hold, an id holding XML white
///    space ([`EditError::InvalidReference`]), then one that names no
///    element ([`EditError::ReferenceNotFound`]), then one of a kind the end
///    does not admit ([`EditError::WrongReferenceType`]), each in list
///    order.
///
/// The narrower rules of §11.15, §11.17 and §11.18, that the ends of some
/// relationships be `ArtifactReference`s, are content: they are not refused,
/// and [`validation::validate`](crate::validation::validate) reports them.
/// The element is in [`changed_ids`](super::Mutation::changed_ids) when the
/// end changes; the referenced elements are not.
///
/// An end set to one or more ids replaces an attribute of the end's name
/// that the element holds as preserved content, such as a `metaClaim=""`
/// read from a file, which held no id (ADR 0033): the edit is the repair, so
/// the element is then changed even if the end held those ids already.
/// [`AddReference`] does the same.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, SetReferences};
/// use sacm::model::{Document, ElementId, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// for (id, text) in [("c1", "The brakes are safe"), ("m1", "The tests are representative")] {
///     edit::apply(&mut doc, CreateClaim::new("p", name(text)).with_id(id))?;
/// }
///
/// let set = edit::apply(&mut doc, SetReferences::new("c1", ReferenceEnd::MetaClaim, ["m1"]))?;
/// assert_eq!(set.changed_ids(), [ElementId::from("c1")]);
/// assert_eq!(doc.get("c1").map(|c1| c1.meta_claims().len()), Some(1));
///
/// // A metaClaim must name a Claim of the document.
/// let missing = SetReferences::new("c1", ReferenceEnd::MetaClaim, ["m9"]);
/// let error = edit::apply(&mut doc, missing).unwrap_err();
/// assert!(matches!(error, EditError::ReferenceNotFound { end: "metaClaim", .. }));
///
/// edit::apply(&mut doc, SetReferences::clear("c1", ReferenceEnd::MetaClaim))?;
/// assert_eq!(doc.get("c1").map(|c1| c1.meta_claims().len()), Some(0));
/// # Ok::<(), EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SetReferences {
    element: ElementId,
    end: ReferenceEnd,
    ids: Vec<ElementId>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    hrefs: Vec<Href>,
}

/// What an operation on one value of a reference end names: an element of
/// the document by its id, or an element of another document by an href
/// (ADR 0040 amendment 1, A5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Link {
    /// An id of the document.
    Id(ElementId),
    /// A cross-document reference.
    Href(Href),
}

impl Link {
    /// The id, or `None` for an href.
    pub(crate) fn id(&self) -> Option<&ElementId> {
        match self {
            Self::Id(id) => Some(id),
            Self::Href(_) => None,
        }
    }

    /// The href, or `None` for an id.
    pub(crate) fn href(&self) -> Option<&Href> {
        match self {
            Self::Id(_) => None,
            Self::Href(href) => Some(href),
        }
    }

    /// The link of a JSON object that gives `id` under the key `id_key` or
    /// `href` under `href_key`: exactly one of them (ADR 0040 amendment 1,
    /// A7).
    #[cfg(feature = "serde")]
    pub(crate) fn from_json(
        id: Option<ElementId>,
        href: Option<Href>,
        (id_key, href_key): (&str, &str),
    ) -> Result<Self, String> {
        match (id, href) {
            (Some(id), None) => Ok(Self::Id(id)),
            (None, Some(href)) => Ok(Self::Href(href)),
            (Some(_), Some(_)) => Err(format!(
                "both `{id_key}` and `{href_key}` are given: exactly one is needed"
            )),
            (None, None) => Err(format!(
                "neither `{id_key}` nor `{href_key}` is given: exactly one is needed"
            )),
        }
    }
}

/// Reads a key that, when it is given, holds a value: `null` is refused.
#[cfg(feature = "serde")]
pub(super) fn given<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// The JSON of an [`AddReference`] or a [`RemoveReference`]: `element`,
/// `end`, and exactly one of `id` and `href` (ADR 0040 amendment 1, A7).
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LinkRecord {
    element: ElementId,
    end: ReferenceEnd,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "given"
    )]
    id: Option<ElementId>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "given"
    )]
    href: Option<Href>,
}

#[cfg(feature = "serde")]
impl LinkRecord {
    /// The record of an operation on `link` in the end `end` of `element`.
    fn new(element: ElementId, end: ReferenceEnd, link: Link) -> Self {
        let (id, href) = match link {
            Link::Id(id) => (Some(id), None),
            Link::Href(href) => (None, Some(href)),
        };
        Self {
            element,
            end,
            id,
            href,
        }
    }

    /// The element, the end and the link it gives.
    fn into_parts(self) -> Result<(ElementId, ReferenceEnd, Link), String> {
        let link = Link::from_json(self.id, self.href, ("id", "href"))?;
        Ok((self.element, self.end, link))
    }
}

impl SetReferences {
    /// Sets the end `end` of `element` to hold `ids`, in the order given,
    /// repeats included.
    #[must_use]
    pub fn new(
        element: impl Into<ElementId>,
        end: ReferenceEnd,
        ids: impl IntoIterator<Item = impl Into<ElementId>>,
    ) -> Self {
        Self {
            element: element.into(),
            end,
            ids: ids.into_iter().map(Into::into).collect(),
            hrefs: Vec::new(),
        }
    }

    /// Empties the end `end` of `element`: the same as [`new`](Self::new)
    /// with no ids: it removes the cross-document references too.
    #[must_use]
    pub fn clear(element: impl Into<ElementId>, end: ReferenceEnd) -> Self {
        Self {
            element: element.into(),
            end,
            ids: Vec::new(),
            hrefs: Vec::new(),
        }
    }

    /// These references with `hrefs` as the end's cross-document
    /// references, in order, repeats included, replacing any given before
    /// (LIB-EDIT-020; ADR 0040 amendment 1).
    ///
    /// The end then holds the ids and these hrefs, and nothing else. Its
    /// bounds count both: a lower bound the distinct ids plus the distinct
    /// hrefs, an upper bound every value. An href is never checked: the
    /// element it names need not exist, and its type is not compared with
    /// the end's.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackageBinding, CreateArgumentPackageInterface,
    ///     CreateArgumentPackage, SetReferences};
    /// use sacm::model::{Document, Href, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Vehicle")).with_id("vehicle"))?;
    /// let interface = CreateArgumentPackageInterface::root(name("Vehicle interface"), "vehicle");
    /// edit::apply(&mut doc, interface.with_id("vehicle_api"))?;
    /// let remote = Href::new("braking.sacm.xmi#braking_api")?;
    /// let other = Href::new("steering.sacm.xmi#steering_api")?;
    /// let binding = CreateArgumentPackageBinding::root(name("Binding"), ["vehicle_api"])
    ///     .with_participant_package_hrefs([remote.clone()]);
    /// edit::apply(&mut doc, binding.with_id("binding"))?;
    ///
    /// let end = ReferenceEnd::ParticipantPackage;
    /// let set = SetReferences::new("binding", end, ["vehicle_api"])
    ///     .with_hrefs([remote.clone(), other.clone()]);
    /// edit::apply(&mut doc, set)?;
    /// assert_eq!(doc.get("binding").map(|b| b.hrefs(end)), Some([remote, other].as_slice()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn with_hrefs(self, hrefs: impl IntoIterator<Item = Href>) -> Self {
        Self {
            hrefs: hrefs.into_iter().collect(),
            ..self
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The end to set.
    #[must_use]
    pub fn end(&self) -> ReferenceEnd {
        self.end
    }

    /// The ids to set, in order: empty for [`clear`](Self::clear).
    #[must_use]
    pub fn ids(&self) -> &[ElementId] {
        &self.ids
    }

    /// The cross-document references to set, in order: empty unless
    /// [`with_hrefs`](Self::with_hrefs) gave some.
    #[must_use]
    pub fn hrefs(&self) -> &[Href] {
        &self.hrefs
    }

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let found = owner(document, &self.element, self.end)?;
        let held = found.references(self.end);
        // The end then holds these ids and these `href`s (ADR 0040 D7,
        // amendment 1 A5); an `href` is never checked (D6).
        check_bounds(self.end, &self.ids, &self.hrefs)?;
        check_targets(document, found.kind(), &[(self.end, &self.ids, held)])?;
        // Setting the ids held still changes the element if it holds the end
        // as a preserved attribute too, which the set replaces (ADR 0033),
        // or other `href`s, which the set replaces.
        let replaces = !self.ids.is_empty()
            && document.has_preserved_attribute(&self.element, self.end.name());
        let relinks = found.hrefs(self.end) != self.hrefs.as_slice();
        Ok(changing(
            &self.element,
            held != self.ids.as_slice() || replaces || relinks,
            Value::Replaced(self.end, self.ids.clone(), self.hrefs.clone()),
        ))
    }
}

/// Appends one id to a reference end of an element.
///
/// It follows the rules of [`SetReferences`] for the end with the id added:
/// it refuses an unknown element ([`EditError::ElementNotFound`]), the
/// derived `expression` ([`EditError::DerivedEnd`]), a kind that does not
/// own the end ([`EditError::FeatureNotApplicable`]), an end
/// that would hold fewer distinct ids than it needs, such as a binding read
/// with one participant that is given it again
/// ([`EditError::TooFewReferences`]; give it two with [`SetReferences`]), or
/// more ids than it allows, such as a second `target`
/// ([`EditError::TooManyReferences`]), and then an id the end does not hold
/// already that holds white space, names no element or names an element of
/// a kind the end does not admit. An id the end holds is appended again,
/// and [`validation::validate`](crate::validation::validate) warns about the
/// repeat (SACM23-VAL-022). Apply it with [`edit::apply`](super::apply).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, AddReference, CreateArgumentPackage, CreateAssertedInference, CreateClaim};
/// use sacm::model::{Document, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// for id in ["c1", "c2", "c3"] {
///     edit::apply(&mut doc, CreateClaim::new("p", name(id)).with_id(id))?;
/// }
/// edit::apply(&mut doc, CreateAssertedInference::new("p", name("I1"), ["c2"], "c1").with_id("i1"))?;
///
/// edit::apply(&mut doc, AddReference::new("i1", ReferenceEnd::Source, "c3"))?;
/// assert_eq!(doc.get("i1").map(|i1| i1.sources().len()), Some(2));
///
/// // An asserted relationship has exactly one target.
/// let error = edit::apply(&mut doc, AddReference::new("i1", ReferenceEnd::Target, "c3")).unwrap_err();
/// assert_eq!(error.to_string(), "the target is given 2 ids, and SACM allows at most 1");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
///
/// [`AddReference::href`] appends a cross-document reference instead
/// (LIB-EDIT-020; ADR 0040 amendment 1): the same first three refusals, then
/// the end's bounds over its ids and its hrefs with the new one. An href is
/// never checked, so it may name nothing, and one the end holds is appended
/// again.
///
/// ```
/// use sacm::edit::{self, AddReference, CreateArgumentPackage, CreateClaim};
/// use sacm::model::{Document, Href, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
///
/// let remote = Href::new("other.sacm.xmi#m1")?;
/// let add = AddReference::href("c1", ReferenceEnd::MetaClaim, remote.clone());
/// assert_eq!((add.id(), add.cross_document()), (None, Some(&remote)));
/// edit::apply(&mut doc, add)?;
/// let c1 = doc.get("c1").ok_or("no c1")?;
/// assert_eq!(c1.hrefs(ReferenceEnd::MetaClaim), [remote]);
/// assert!(c1.meta_claims().is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "LinkRecord", into = "LinkRecord")
)]
pub struct AddReference {
    element: ElementId,
    end: ReferenceEnd,
    link: Link,
}

#[cfg(feature = "serde")]
impl TryFrom<LinkRecord> for AddReference {
    type Error = String;

    fn try_from(record: LinkRecord) -> Result<Self, String> {
        let (element, end, link) = record.into_parts()?;
        Ok(Self { element, end, link })
    }
}

#[cfg(feature = "serde")]
impl From<AddReference> for LinkRecord {
    fn from(operation: AddReference) -> Self {
        Self::new(operation.element, operation.end, operation.link)
    }
}

impl AddReference {
    /// Appends `id` to the end `end` of `element`.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, end: ReferenceEnd, id: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            end,
            link: Link::Id(id.into()),
        }
    }

    /// Appends the cross-document reference `href` to the end `end` of
    /// `element`, after the hrefs the end holds (LIB-EDIT-020; ADR 0040
    /// amendment 1).
    #[must_use]
    pub fn href(element: impl Into<ElementId>, end: ReferenceEnd, href: Href) -> Self {
        Self {
            element: element.into(),
            end,
            link: Link::Href(href),
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The end to append to.
    #[must_use]
    pub fn end(&self) -> ReferenceEnd {
        self.end
    }

    /// The id to append, or `None` for an href.
    #[must_use]
    pub fn id(&self) -> Option<&ElementId> {
        self.link.id()
    }

    /// The href to append, or `None` for an id.
    #[doc(alias = "href")]
    #[must_use]
    pub fn cross_document(&self) -> Option<&Href> {
        self.link.href()
    }

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let found = owner(document, &self.element, self.end)?;
        let held = found.references(self.end);
        let value = match &self.link {
            Link::Id(id) => {
                let mut ids = held.to_vec();
                ids.push(id.clone());
                check_bounds(self.end, &ids, found.hrefs(self.end))?;
                check_targets(
                    document,
                    found.kind(),
                    &[(self.end, slice::from_ref(id), held)],
                )?;
                Value::References(self.end, ids)
            }
            // An `href` is never checked (ADR 0040 D6): only the bounds.
            Link::Href(href) => {
                let mut hrefs = found.hrefs(self.end).to_vec();
                hrefs.push(href.clone());
                check_bounds(self.end, held, &hrefs)?;
                Value::Hrefs(self.end, hrefs)
            }
        };
        Ok(Plan::Set {
            element: &self.element,
            value,
        })
    }
}

/// Removes every occurrence of one id from a reference end of an element.
///
/// It refuses, in order: an unknown element
/// ([`EditError::ElementNotFound`]), the derived `expression`
/// ([`EditError::DerivedEnd`]), a kind that does not own the end
/// ([`EditError::FeatureNotApplicable`]), an id the end does not hold
/// ([`EditError::ReferenceNotHeld`]), and an end that would hold fewer ids
/// than it needs, such as an asserted relationship's last source or its
/// target ([`EditError::TooFewReferences`]). The id need not name an
/// element: removing a dangling id is how a file's dangling reference is
/// repaired, and once no reference holds it, an edit may generate it as a
/// new element's id. Apply it with [`edit::apply`](super::apply).
///
/// Like [`SetReferences`] and [`AddReference`], a removal that leaves the
/// end holding one or more ids replaces an attribute of the end's name that
/// the element holds as preserved content, which held no id (ADR 0033).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, RemoveReference};
/// use sacm::model::ReferenceEnd;
/// use sacm::validation;
/// use sacm::xmi::{self, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
///   <name content="Arguments"/>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" metaClaim="gone">
///     <name content="The brakes are safe"/>
///   </argumentElement>
/// </sacm:ArgumentPackage>"#;
/// let mut doc = xmi::read(input, &ReadOptions::default())?.into_document();
/// assert!(!validation::validate(&doc).is_empty()); // the dangling metaClaim
///
/// edit::apply(&mut doc, RemoveReference::new("c1", ReferenceEnd::MetaClaim, "gone"))?;
/// assert!(validation::validate(&doc).is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [`RemoveReference::href`] removes every occurrence of a cross-document
/// reference instead (LIB-EDIT-020; ADR 0040 amendment 1): the same first
/// three refusals, then an href the end does not hold
/// ([`EditError::HrefNotHeld`]; hrefs are compared as strings), then the
/// end's bounds over what is left.
///
/// ```
/// use sacm::edit::{self, AddReference, CreateArgumentPackage, CreateClaim, RemoveReference};
/// use sacm::model::{Document, Href, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
/// let remote = Href::new("other.sacm.xmi#m1")?;
/// let end = ReferenceEnd::MetaClaim;
/// edit::apply(&mut doc, AddReference::href("c1", end, remote.clone()))?;
///
/// let remove = RemoveReference::href("c1", end, remote.clone());
/// assert_eq!((remove.id(), remove.cross_document()), (None, Some(&remote)));
/// edit::apply(&mut doc, remove)?;
/// assert!(doc.get("c1").is_some_and(|c1| c1.hrefs(end).is_empty()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "LinkRecord", into = "LinkRecord")
)]
pub struct RemoveReference {
    element: ElementId,
    end: ReferenceEnd,
    link: Link,
}

#[cfg(feature = "serde")]
impl TryFrom<LinkRecord> for RemoveReference {
    type Error = String;

    fn try_from(record: LinkRecord) -> Result<Self, String> {
        let (element, end, link) = record.into_parts()?;
        Ok(Self { element, end, link })
    }
}

#[cfg(feature = "serde")]
impl From<RemoveReference> for LinkRecord {
    fn from(operation: RemoveReference) -> Self {
        Self::new(operation.element, operation.end, operation.link)
    }
}

impl RemoveReference {
    /// Removes every occurrence of `id` from the end `end` of `element`.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, end: ReferenceEnd, id: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            end,
            link: Link::Id(id.into()),
        }
    }

    /// Removes every occurrence of the cross-document reference `href` from
    /// the end `end` of `element` (LIB-EDIT-020; ADR 0040 amendment 1).
    #[must_use]
    pub fn href(element: impl Into<ElementId>, end: ReferenceEnd, href: Href) -> Self {
        Self {
            element: element.into(),
            end,
            link: Link::Href(href),
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The end to remove from.
    #[must_use]
    pub fn end(&self) -> ReferenceEnd {
        self.end
    }

    /// The id to remove, or `None` for an href.
    #[must_use]
    pub fn id(&self) -> Option<&ElementId> {
        self.link.id()
    }

    /// The href to remove, or `None` for an id.
    #[doc(alias = "href")]
    #[must_use]
    pub fn cross_document(&self) -> Option<&Href> {
        self.link.href()
    }

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let found = owner(document, &self.element, self.end)?;
        let held = found.references(self.end);
        let held_hrefs = found.hrefs(self.end);
        let value = match &self.link {
            Link::Id(id) => {
                if !held.contains(id) {
                    return Err(EditError::ReferenceNotHeld {
                        end: self.end.name(),
                        id: id.clone(),
                    });
                }
                let ids: Vec<ElementId> = held.iter().filter(|one| *one != id).cloned().collect();
                check_bounds(self.end, &ids, held_hrefs)?;
                Value::References(self.end, ids)
            }
            Link::Href(href) => {
                if !held_hrefs.contains(href) {
                    return Err(EditError::HrefNotHeld {
                        end: self.end.name(),
                        href: href.clone(),
                    });
                }
                let hrefs: Vec<Href> = held_hrefs
                    .iter()
                    .filter(|one| *one != href)
                    .cloned()
                    .collect();
                check_bounds(self.end, held, &hrefs)?;
                Value::Hrefs(self.end, hrefs)
            }
        };
        Ok(Plan::Set {
            element: &self.element,
            value,
        })
    }
}

/// The element `element` of `document`, once it is known, the end is not
/// the derived `expression` and its kind owns `end`: the first checks of a
/// reference edit.
fn owner<'d>(
    document: &'d Document,
    element: &ElementId,
    end: ReferenceEnd,
) -> Result<SacmElement<'d>, EditError> {
    let found = edited(document, element, end.name(), |kind| end.applies_to(kind))?;
    if end.is_derived() {
        // Its ids are the name's and texts' (SACM23-BASE-009, ADR 0038 D3).
        return Err(EditError::DerivedEnd {
            element: element.clone(),
            kind: found.kind(),
            end: end.name(),
        });
    }
    Ok(found)
}

/// The `expression`s of the `ExpressionLangStrings` `values` hold, in order,
/// once each has one: an `ExpressionLangString` with no `expression` is
/// [`EditError::TooFewReferences`] of the end `expression`, since §8.4 gives
/// `expression[1]` and a required end is structural (D-VAL-LAYER, ADR 0038
/// D3).
pub(super) fn expression_ids<'v>(
    values: impl IntoIterator<Item = &'v LangString>,
) -> Result<Vec<ElementId>, EditError> {
    let mut ids = Vec::new();
    for value in values {
        if !value.is_expression_lang_string() {
            continue;
        }
        // An href counts as the expression being present, and is never
        // checked (ADR 0040 amendment 1, A4).
        if value.expression_href().is_some() {
            continue;
        }
        let Some(expression) = value.expression() else {
            return Err(EditError::TooFewReferences {
                end: ReferenceEnd::Expression.name(),
                given: 0,
                required: 1,
            });
        };
        ids.push(expression.clone());
    }
    Ok(ids)
}

/// Checks the `ExpressionLangStrings` `values` hold, as an operation that
/// creates or sets them does (SACM23-BASE-009): [`expression_ids`], then
/// each `expression` not in `held`, as [`check_targets`] checks a new id of
/// the end `expression`. `held` is the element's `expression` end for a set,
/// and empty for a create.
pub(super) fn check_expressions<'v>(
    document: &Document,
    values: impl IntoIterator<Item = &'v LangString>,
    held: &[ElementId],
) -> Result<(), EditError> {
    let ids = expression_ids(values)?;
    // Every element holds the derived end, typed alike whatever its kind.
    check_targets(
        document,
        ElementKind::Claim,
        &[(ReferenceEnd::Expression, &ids, held)],
    )
}

/// The number of distinct values an end holding `ids` and `hrefs` holds:
/// what a lower bound counts (ADR 0039 D3, ADR 0040 D5), so that
/// `["a", "a"]` is one participant package. An id and an `href` are never
/// the same value, and `href`s are compared as strings.
pub(super) fn distinct_count(ids: &[ElementId], hrefs: &[Href]) -> usize {
    ids.iter().collect::<BTreeSet<_>>().len() + hrefs.iter().collect::<BTreeSet<_>>().len()
}

/// Refuses the ids `ids`, beside the cross-document references `hrefs` the
/// end keeps, on `end` when they are fewer
/// ([`EditError::TooFewReferences`]) or more
/// ([`EditError::TooManyReferences`]) than its bounds allow. The lower bound
/// counts the distinct values and the upper bound every value, ids and
/// `href`s alike (ADR 0039 D3, ADR 0040 D5); the two counts differ only for
/// an end whose lower bound is above 1.
pub(super) fn check_bounds(
    end: ReferenceEnd,
    ids: &[ElementId],
    hrefs: &[Href],
) -> Result<(), EditError> {
    let required = end.lower_bound();
    let distinct = distinct_count(ids, hrefs);
    if distinct < required {
        return Err(EditError::TooFewReferences {
            end: end.name(),
            given: distinct,
            required,
        });
    }
    let given = ids.len() + hrefs.len();
    match end.upper_bound() {
        Some(allowed) if given > allowed => Err(EditError::TooManyReferences {
            end: end.name(),
            given,
            allowed,
        }),
        _ => Ok(()),
    }
}

/// Checks the ids each end of `ends` is to hold, skipping those it already
/// holds (ADR 0029): each `(end, ids, held)` gives the ids and the ids the
/// end holds now, empty for a create.
///
/// Three passes, each over every end in turn and its ids in order, and the
/// first id that fails is refused: one holding XML white space
/// ([`EditError::InvalidReference`]), since it would be written as an
/// IDREFS attribute of several ids; then one that names no element
/// ([`EditError::ReferenceNotFound`]); then one that names an element of a
/// kind the end's declared type does not admit
/// ([`EditError::WrongReferenceType`], ADR 0028). So every id is checked for
/// existence before any is checked for type: the type the end declares on
/// `holder`, the kind of the element that holds or is to hold the ends (ADR
/// 0039 D1). Changes nothing.
pub(super) fn check_targets(
    document: &Document,
    holder: ElementKind,
    ends: &[(ReferenceEnd, &[ElementId], &[ElementId])],
) -> Result<(), EditError> {
    let new: Vec<(ReferenceEnd, Vec<&ElementId>)> = ends
        .iter()
        .map(|&(end, ids, held)| {
            let held: BTreeSet<&ElementId> = held.iter().collect();
            (end, ids.iter().filter(|id| !held.contains(id)).collect())
        })
        .collect();
    for (end, ids) in &new {
        if let Some(spaced) = ids
            .iter()
            .find(|id| id.as_str().contains([' ', '\t', '\n', '\r']))
        {
            return Err(EditError::InvalidReference {
                end: end.name(),
                id: (*spaced).clone(),
            });
        }
    }
    for (end, ids) in &new {
        if let Some(missing) = ids.iter().find(|id| !document.contains(**id)) {
            return Err(EditError::ReferenceNotFound {
                end: end.name(),
                id: (*missing).clone(),
            });
        }
    }
    for (end, ids) in &new {
        let Some(declared) = end.target_on(holder) else {
            continue;
        };
        let wrong = ids.iter().find_map(|id| {
            let kind = document.get(*id)?.kind();
            (!declared.admits(kind)).then_some((*id, kind))
        });
        if let Some((id, kind)) = wrong {
            return Err(EditError::WrongReferenceType {
                end: end.name(),
                id: id.clone(),
                kind,
                expected: declared.name(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SetReferences;
    use crate::edit;
    use crate::model::{ElementId, ReferenceEnd};
    use crate::xmi::{self, ReadOptions};

    /// Setting the ids an end holds still changes the element when the end
    /// also holds hrefs, which the set removes (ADR 0040 D7).
    #[test]
    fn setting_the_held_ids_removes_the_hrefs_and_changes_the_element() {
        let input =
            br#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="case"/>
  <argumentPackage xmi:id="p"><name content="p"/></argumentPackage>
  <argumentPackage xmi:id="q"><name content="q"/></argumentPackage>
  <argumentPackage xmi:type="sacm:ArgumentPackageBinding" xmi:id="b" participantPackage="p q">
    <name content="b"/>
  </argumentPackage>
</sacm:AssuranceCasePackage>"#;
        let mut doc = match xmi::read(input, &ReadOptions::default()) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("the input does not read: {error}"),
        };
        let end = ReferenceEnd::ParticipantPackage;
        let unchanged = edit::apply(&mut doc, SetReferences::new("b", end, ["p", "q"]));
        assert!(
            unchanged.is_ok_and(|mutation| mutation.changed_ids().is_empty()),
            "setting the held ids with no href changed the element"
        );
        let with_href = String::from_utf8_lossy(input).replacen(
            "<name content=\"b\"/>",
            "<name content=\"b\"/><participantPackage href=\"o.xmi#x\"/>",
            1,
        );
        let with_href = with_href.replacen(" participantPackage=\"p q\"", "", 1).replacen(
            "<participantPackage href",
            "<participantPackage xmi:idref=\"p\"/><participantPackage xmi:idref=\"q\"/><participantPackage href",
            1,
        );
        let mut doc = match xmi::read(with_href.as_bytes(), &ReadOptions::default()) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("the input with an href does not read: {error}"),
        };
        let mutation = match edit::apply(&mut doc, SetReferences::new("b", end, ["p", "q"])) {
            Ok(mutation) => mutation,
            Err(error) => panic!("the set was refused: {error}"),
        };
        assert_eq!(mutation.changed_ids(), [ElementId::from("b")]);
        assert!(doc.get("b").is_some_and(|b| b.hrefs(end).is_empty()));
    }
}
