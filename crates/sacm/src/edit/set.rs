//! The operations that set a feature of an existing element: its name, gid,
//! flags, `assertionDeclaration`, and a `UtilityElement`'s `content` and
//! `key` (LIB-EDIT-008, LIB-EDIT-012, ADR 0029).

use super::EditError;
use crate::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString,
    MultiLangString, SacmElement, Text, TextRole,
};

/// Sets the `name` (SACM 2.3 §8.6 `name:LangString[1]`) of a `ModelElement`,
/// replacing the name it has, or giving one to an element read without one.
///
/// The name is held exactly as given, `LangString::default()` included, and
/// keeps the `xmi:id` of the name it replaces
/// ([`SacmElement::name_id`](crate::model::SacmElement::name_id)). A name
/// holding a character XML cannot represent is held, and
/// [`xmi::write`](crate::xmi::write) refuses it. Apply it with
/// [`edit::apply`](super::apply). It refuses an unknown element as
/// [`EditError::ElementNotFound`], then a `UtilityElement`, which has no
/// name, as [`EditError::FeatureNotApplicable`]. The element is in
/// [`changed_ids`](super::Mutation::changed_ids) when its name changes.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetName};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("The brakes are safe")).with_id("c1"))?;
///
/// let renamed = edit::apply(&mut doc, SetName::new("c1", name("The brakes are adequate")))?;
/// assert_eq!(renamed.changed_ids(), [ElementId::from("c1")]);
/// assert_eq!(doc.get("c1").and_then(|c1| c1.name()), Some(&name("The brakes are adequate")));
///
/// // The same name again changes nothing.
/// let again = edit::apply(&mut doc, SetName::new("c1", name("The brakes are adequate")))?;
/// assert!(again.changed_ids().is_empty());
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetName {
    element: ElementId,
    name: LangString,
}

impl SetName {
    /// Sets the name of `element` to `name`.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            element: element.into(),
            name,
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The name to set.
    #[must_use]
    pub fn name(&self) -> &LangString {
        &self.name
    }

    /// Checks and applies the operation, and returns the ids it changed.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        let target = edited(document, &self.element, "name", |kind| {
            kind.is_a(AbstractClass::ModelElement)
        })?;
        if target.name() == Some(&self.name) {
            return Ok(Vec::new());
        }
        document.replace_name(&self.element, self.name.clone());
        Ok(vec![self.element.clone()])
    }
}

/// Sets or removes the `gid` (SACM 2.3 §8.2 `gid:String[0..1]`) of any
/// element.
///
/// [`new`](Self::new) sets it, exactly as given: `""` is a value, not a
/// removal. [`remove`](Self::remove) removes it. A gid is not an `xmi:id`,
/// so no value is refused, a gid another element holds included;
/// [`validation::validate`](crate::validation::validate) reports a
/// duplicate (SACM-ID-003). A gid holding a character XML cannot represent
/// is held, and [`xmi::write`](crate::xmi::write) refuses it. Apply it with
/// [`edit::apply`](super::apply); it refuses only an unknown element, as
/// [`EditError::ElementNotFound`].
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, SetGid};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, SetGid::new("p", "urn:example:arguments"))?;
/// assert_eq!(doc.get("p").and_then(|p| p.gid()), Some("urn:example:arguments"));
/// edit::apply(&mut doc, SetGid::remove("p"))?;
/// assert_eq!(doc.get("p").and_then(|p| p.gid()), None);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetGid {
    element: ElementId,
    gid: Option<String>,
}

impl SetGid {
    /// Sets the gid of `element` to `gid`, exactly.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, gid: impl Into<String>) -> Self {
        Self {
            element: element.into(),
            gid: Some(gid.into()),
        }
    }

    /// Removes the gid of `element`.
    #[must_use]
    pub fn remove(element: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            gid: None,
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The gid to set, or `None` to remove it.
    #[must_use]
    pub fn gid(&self) -> Option<&str> {
        self.gid.as_deref()
    }

    /// Checks and applies the operation, and returns the ids it changed.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        let target = edited(document, &self.element, "gid", |_| true)?;
        if target.gid() == self.gid.as_deref() {
            return Ok(Vec::new());
        }
        let set = document.set_gid(&self.element, self.gid.clone());
        debug_assert!(set.is_ok(), "the element exists: {set:?}");
        Ok(vec![self.element.clone()])
    }
}

/// Sets a Boolean flag of an element: `isCitation` or `isAbstract` of any
/// element (SACM 2.3 §8.2), or `isCounter` of an asserted relationship
/// (§11.13).
///
/// A flag is set independently of the reference it goes with:
/// `isCitation` of the `citedElement`, `isAbstract` of the `abstractForm`
/// (ADR 0029). [`validation::validate`](crate::validation::validate)
/// reports a combination §8.2 rules out (SACM-CITE-001, SACM-ABS-001).
/// Apply it with [`edit::apply`](super::apply). It refuses an unknown
/// element as [`EditError::ElementNotFound`], then `isCounter` on a kind
/// that is no `AssertedRelationship` as [`EditError::FeatureNotApplicable`].
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetFlag};
/// use sacm::model::{Document, Flag, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("{System} is safe")).with_id("pattern"))?;
/// edit::apply(&mut doc, SetFlag::new("pattern", Flag::IsAbstract, true))?;
/// assert!(doc.get("pattern").is_some_and(|claim| claim.is_abstract()));
///
/// // A Claim is no asserted relationship, so it has no isCounter.
/// let refused = edit::apply(&mut doc, SetFlag::new("pattern", Flag::IsCounter, true)).unwrap_err();
/// assert_eq!(refused.to_string(), "the Claim \"pattern\" has no isCounter");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetFlag {
    element: ElementId,
    flag: Flag,
    value: bool,
}

impl SetFlag {
    /// Sets the flag `flag` of `element` to `value`.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, flag: Flag, value: bool) -> Self {
        Self {
            element: element.into(),
            flag,
            value,
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The flag to set.
    #[must_use]
    pub fn flag(&self) -> Flag {
        self.flag
    }

    /// The value to set it to.
    #[must_use]
    pub fn value(&self) -> bool {
        self.value
    }

    /// Checks and applies the operation, and returns the ids it changed.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        let flag = self.flag;
        let target = edited(document, &self.element, flag.name(), |kind| {
            flag.owner().admits(kind)
        })?;
        if target.flag(flag) == self.value {
            return Ok(Vec::new());
        }
        let set = document.set_flag(&self.element, flag, self.value);
        debug_assert!(set.is_ok(), "the element exists and has the flag: {set:?}");
        Ok(vec![self.element.clone()])
    }
}

/// Sets the `assertionDeclaration` (SACM 2.3 §11.10) of an Assertion: a
/// Claim or an asserted relationship.
///
/// Any of the six literals is accepted, `asCited` on an element that is no
/// citation included: that is content, for
/// [`validation::validate`](crate::validation::validate) (D-VAL-LAYER).
/// Apply it with [`edit::apply`](super::apply). It refuses an unknown
/// element as [`EditError::ElementNotFound`], then a kind that is no
/// Assertion as [`EditError::FeatureNotApplicable`].
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetAssertionDeclaration};
/// use sacm::model::{AssertionDeclaration, Document, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("The pump is safe")).with_id("c1"))?;
/// edit::apply(&mut doc, SetAssertionDeclaration::new("c1", AssertionDeclaration::NeedsSupport))?;
/// let c1 = doc.get("c1").expect("just created");
/// assert_eq!(c1.assertion_declaration(), Some(AssertionDeclaration::NeedsSupport));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetAssertionDeclaration {
    element: ElementId,
    declaration: AssertionDeclaration,
}

impl SetAssertionDeclaration {
    /// Sets the `assertionDeclaration` of `element` to `declaration`.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, declaration: AssertionDeclaration) -> Self {
        Self {
            element: element.into(),
            declaration,
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The declaration to set.
    #[must_use]
    pub fn declaration(&self) -> AssertionDeclaration {
        self.declaration
    }

    /// Checks and applies the operation, and returns the ids it changed.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        let target = edited(document, &self.element, "assertionDeclaration", |kind| {
            kind.is_a(AbstractClass::Assertion)
        })?;
        if target.assertion_declaration() == Some(self.declaration) {
            return Ok(Vec::new());
        }
        let set = document.set_declaration(&self.element, self.declaration);
        debug_assert!(
            set.is_ok(),
            "the element exists and is an Assertion: {set:?}"
        );
        Ok(vec![self.element.clone()])
    }
}

/// Sets or removes the `content` (SACM 2.3 §8.7
/// `content:MultiLangString[0..1]`) of a `UtilityElement`: a Description,
/// `ImplementationConstraint`, Note or `TaggedValue`.
///
/// [`new`](Self::new) replaces the whole content with any
/// `MultiLangString`, held exactly: an empty one, or one with a repeated
/// language, is accepted, and
/// [`validation::validate`](crate::validation::validate) reports it
/// (SACM23-VAL-010). [`remove`](Self::remove) removes it. A replaced content
/// keeps the `xmi:id` of its `content` element, and each value keeps the id
/// of the value at its position; the ids of values beyond the new length
/// are freed, as are all of them on a removal (ADR 0029). A content holding
/// a character XML cannot represent is held, and
/// [`xmi::write`](crate::xmi::write) refuses it. Apply it with
/// [`edit::apply`](super::apply). It refuses an unknown element as
/// [`EditError::ElementNotFound`], then a kind that is no `UtilityElement`
/// as [`EditError::FeatureNotApplicable`]. The `UtilityElement`, not its
/// owner, is in [`changed_ids`](super::Mutation::changed_ids).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateNote, SetContent};
/// use sacm::model::{Document, LangString, MultiLangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateNote::new("p", LangString::new("Draft")).with_id("n1"))?;
///
/// let reviewed: MultiLangString = [
///     LangString::new("Reviewed").with_lang("en"),
///     LangString::new("Granskad").with_lang("sv"),
/// ]
/// .into_iter()
/// .collect();
/// edit::apply(&mut doc, SetContent::new("n1", reviewed.clone()))?;
/// assert_eq!(doc.get("n1").and_then(|n1| n1.content()), Some(&reviewed));
/// edit::apply(&mut doc, SetContent::remove("n1"))?;
/// assert_eq!(doc.get("n1").and_then(|n1| n1.content()), None);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetContent {
    element: ElementId,
    content: Option<MultiLangString>,
}

impl SetContent {
    /// Sets the content of `element` to `content`. A [`LangString`] converts
    /// into a content of one value.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, content: impl Into<MultiLangString>) -> Self {
        Self {
            element: element.into(),
            content: Some(content.into()),
        }
    }

    /// Removes the content of `element`.
    #[must_use]
    pub fn remove(element: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            content: None,
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The content to set, or `None` to remove it.
    #[must_use]
    pub fn content(&self) -> Option<&MultiLangString> {
        self.content.as_ref()
    }

    /// Checks and applies the operation, and returns the ids it changed.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        set_text(
            document,
            &self.element,
            TextRole::Content,
            self.content.as_ref(),
        )
    }
}

/// Sets the `key` (SACM 2.3 §8.12 `key:MultiLangString[1]`) of a
/// `TaggedValue`.
///
/// It replaces the whole key with any `MultiLangString`, held exactly, under
/// the rules of [`SetContent::new`]: an empty key is accepted and reported
/// by [`validation::validate`](crate::validation::validate), and ids are
/// kept by position. A key is required, so there is no form that removes
/// it. Apply it with [`edit::apply`](super::apply). It refuses an unknown
/// element as [`EditError::ElementNotFound`], then a kind that is no
/// `TaggedValue` as [`EditError::FeatureNotApplicable`].
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateTaggedValue, SetKey};
/// use sacm::model::{Document, LangString, MultiLangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// let tagged = CreateTaggedValue::new("p", LangString::new("ASIL"), LangString::new("D"));
/// edit::apply(&mut doc, tagged.with_id("t1"))?;
/// edit::apply(&mut doc, SetKey::new("t1", LangString::new("SIL")))?;
/// assert_eq!(doc.get("t1").and_then(|t1| t1.key()), Some(&MultiLangString::from(LangString::new("SIL"))));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetKey {
    element: ElementId,
    key: MultiLangString,
}

impl SetKey {
    /// Sets the key of `element` to `key`. A [`LangString`] converts into a
    /// key of one value.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, key: impl Into<MultiLangString>) -> Self {
        Self {
            element: element.into(),
            key: key.into(),
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The key to set.
    #[must_use]
    pub fn key(&self) -> &MultiLangString {
        &self.key
    }

    /// Checks and applies the operation, and returns the ids it changed.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        set_text(document, &self.element, TextRole::Key, Some(&self.key))
    }
}

/// Sets the `role` of `element` to `value`, or removes it when `None`,
/// keeping the ids by position, and returns the ids changed: `element`
/// exactly when the value or its ids change.
fn set_text(
    document: &mut Document,
    element: &ElementId,
    role: TextRole,
    value: Option<&MultiLangString>,
) -> Result<Vec<ElementId>, EditError> {
    let target = edited(document, element, role.name(), |kind| role.is_held_by(kind))?;
    let held = target.text(role);
    let text = value.map(|value| Text::keeping_ids(value.clone(), held));
    if held == text.as_ref() {
        return Ok(Vec::new());
    }
    match text {
        Some(text) => {
            let set = document.set_text(element, role, text);
            debug_assert!(set.is_ok(), "the kept ids are the role's own: {set:?}");
        }
        None => document.remove_text(element, role),
    }
    Ok(vec![element.clone()])
}

/// The element `element` of `document`, once it is known
/// ([`EditError::ElementNotFound`]) and its kind has `feature`, by its XMI
/// name ([`EditError::FeatureNotApplicable`]): the first two checks of every
/// operation that edits an element (ADR 0029).
pub(super) fn edited<'d>(
    document: &'d Document,
    element: &ElementId,
    feature: &'static str,
    has_feature: impl FnOnce(ElementKind) -> bool,
) -> Result<SacmElement<'d>, EditError> {
    let Some(target) = document.get(element) else {
        return Err(EditError::ElementNotFound {
            element: element.clone(),
        });
    };
    if !has_feature(target.kind()) {
        return Err(EditError::FeatureNotApplicable {
            element: element.clone(),
            kind: target.kind(),
            feature,
        });
    }
    Ok(target)
}
