//! The operations that set a feature of an existing element.
//!
//! - **Any element's features.** Its name, gid, flags and `assertionDeclaration`, and a
//!   `UtilityElement`'s `content` and `key` (LIB-EDIT-008, LIB-EDIT-012, ADR 0029).
//! - **An `ArtifactAsset`'s values.** Its `version`, `date`, `location`, `startTime` and
//!   `endTime` (SACM23-ART-002, SACM23-ART-004, SACM23-ART-005; ADR 0034 amendment 1).

use std::borrow::Cow;

use super::EditError;
use super::create::plan_create;
use super::plan::{Features, Plan, Value};
use super::references::check_expressions;
use crate::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString,
    MultiLangString, SacmElement, StringAttribute, Text, TextRole,
};

/// The XMI name of the `name` feature, as [`EditError::FeatureNotApplicable`]
/// names it.
pub(super) const NAME: &str = "name";
/// The XMI name of the `gid` feature.
pub(super) const GID: &str = "gid";
/// The XMI name of the `assertionDeclaration` feature, as
/// [`EditError::FeatureNotApplicable`] names it.
pub(super) const ASSERTION_DECLARATION: &str = "assertionDeclaration";
/// The XMI name of the `taggedValue` role (SACM 2.3 §8.6), as
/// [`EditError::FeatureNotApplicable`] names it for [`SetTaggedValue`].
pub(super) const TAGGED_VALUE: &str = "taggedValue";

/// Sets the `name` (SACM 2.3 §8.6 `name:LangString[1]`) of a `ModelElement`,
/// replacing the name it has, or giving one to an element read without one.
///
/// - **Held exactly as given,** `LangString::default()` included. It keeps the `xmi:id`
///   of the name it replaces
///   ([`SacmElement::name_id`](crate::model::SacmElement::name_id)).
/// - **The element** is in [`changed_ids`](super::Mutation::changed_ids) when its name
///   changes.
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// It refuses, in this order:
///
/// | Refused | Refusal |
/// |---|---|
/// | an unknown element | [`EditError::ElementNotFound`] |
/// | a `UtilityElement`, which has no name | [`EditError::FeatureNotApplicable`] |
/// | a name that is an `ExpressionLangString` (§8.4) with no `expression` | [`EditError::TooFewReferences`] |
/// | an `expression` the element does not already hold that names no `Term` or `Expression` of the document | [`EditError::InvalidReference`], [`EditError::ReferenceNotFound`], [`EditError::WrongReferenceType`], end `expression` (SACM23-BASE-009) |
/// | last, a name holding a character XML 1.0 cannot represent | [`EditError::UnrepresentableCharacter`] |
///
/// [`SetContent`], [`SetKey`] and [`SetLocation`] check their values the same way.
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SetName {
    element: ElementId,
    name: LangString,
}

impl SetName {
    /// Sets the name of `element` to `name`.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, name: impl Into<LangString>) -> Self {
        Self {
            element: element.into(),
            name: name.into(),
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

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let target = edited(document, &self.element, NAME, |kind| {
            kind.is_a(AbstractClass::ModelElement)
        })?;
        check_expressions(document, [&self.name], target.expressions())?;
        Ok(changing(
            &self.element,
            target.name() != Some(&self.name),
            Value::Name(&self.name),
        ))
    }
}

/// Sets or removes the `gid` (SACM 2.3 §8.2 `gid:String[0..1]`) of any
/// element.
///
/// - **[`new`](Self::new)** sets it, exactly as given: `""` is a value, not a removal.
///   [`remove`](Self::remove) removes it.
/// - **Not an `xmi:id`.** No value is refused for its content, a gid another element
///   holds included; [`validation::validate`](crate::validation::validate) reports a
///   duplicate (SACM-ID-003).
/// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a gid
///   holding a character XML 1.0 cannot represent is
///   [`EditError::UnrepresentableCharacter`].
/// - **Apply it** with [`edit::apply`](super::apply).
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SetGid {
    element: ElementId,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
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

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let target = edited(document, &self.element, GID, |_| true)?;
        Ok(changing(
            &self.element,
            target.gid() != self.gid.as_deref(),
            Value::Gid(self.gid.as_deref()),
        ))
    }
}

/// Sets a Boolean flag of an element: `isCitation` or `isAbstract` of any element
/// (SACM 2.3 §8.2), or `isCounter` of an asserted relationship (§11.13).
///
/// - **Independent of its reference.** A flag is set independently of the reference it
///   goes with: `isCitation` of the `citedElement`, `isAbstract` of the `abstractForm`
///   (ADR 0029).
/// - **Content is for `validate`.** [`validation::validate`](crate::validation::validate)
///   reports a combination §8.2 rules out (SACM-CITE-001, SACM-ABS-001).
/// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then
///   `isCounter` on a kind that is no `AssertedRelationship` is
///   [`EditError::FeatureNotApplicable`].
/// - **A preserved attribute of the flag's name** (one whose value was no literal,
///   ADR 0035) is replaced by the set. The element then changes even when the flag is
///   set to the value it holds.
/// - **Apply it** with [`edit::apply`](super::apply).
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
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

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let flag = self.flag;
        let target = edited(document, &self.element, flag.name(), |kind| {
            flag.owner().admits(kind)
        })?;
        // Setting the value held still changes the element if it holds an
        // attribute of the flag's name as preserved content, whose value was
        // no literal, which the set replaces (ADR 0035).
        let replaces = document.has_preserved_attribute(&self.element, flag.name());
        Ok(changing(
            &self.element,
            target.flag(flag) != self.value || replaces,
            Value::Flag(flag, self.value),
        ))
    }
}

/// Sets the `assertionDeclaration` (SACM 2.3 §11.10) of an Assertion: a
/// Claim or an asserted relationship.
///
/// - **Any of the six literals is accepted,** `asCited` on an element that is no
///   citation included: that is content, for
///   [`validation::validate`](crate::validation::validate) (D-VAL-LAYER).
/// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind that
///   is no Assertion is [`EditError::FeatureNotApplicable`].
/// - **A preserved `assertionDeclaration` attribute** (one whose value was no literal,
///   ADR 0035) is replaced by the set, as for [`SetFlag`].
/// - **Apply it** with [`edit::apply`](super::apply).
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
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

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let target = edited(document, &self.element, ASSERTION_DECLARATION, |kind| {
            kind.is_a(AbstractClass::Assertion)
        })?;
        // As for a flag: a preserved attribute of the name is replaced.
        let replaces = document.has_preserved_attribute(&self.element, ASSERTION_DECLARATION);
        Ok(changing(
            &self.element,
            target.assertion_declaration() != Some(self.declaration) || replaces,
            Value::Declaration(self.declaration),
        ))
    }
}

/// Sets or removes the `content` (SACM 2.3 §8.7 `content:MultiLangString[0..1]`) of a
/// `UtilityElement`: a Description, `ImplementationConstraint`, Note or `TaggedValue`.
///
/// - **[`new`](Self::new)** replaces the whole content with any `MultiLangString` it
///   does not refuse below, held exactly. [`remove`](Self::remove) removes it.
/// - **Content is not refused.** An empty one, or one with a repeated language, is
///   accepted, and [`validation::validate`](crate::validation::validate) reports it
///   (SACM23-VAL-010).
/// - **Ids are kept by position.** A replaced content keeps the `xmi:id` of its
///   `content` element, and each value keeps the id of the value at its position.
/// - **Freed ids.** The ids of values beyond the new length are freed, as are all of
///   them on a removal (ADR 0029). It refuses to free one that a reference end or XMI
///   metadata names ([`EditError::PartReference`], [`EditError::MetadataReference`];
///   LIB-EDIT-026).
/// - **The `UtilityElement`,** not its owner, is in
///   [`changed_ids`](super::Mutation::changed_ids).
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// It refuses, in this order:
///
/// | Refused | Refusal |
/// |---|---|
/// | an unknown element | [`EditError::ElementNotFound`] |
/// | a kind that is no `UtilityElement` | [`EditError::FeatureNotApplicable`] |
/// | an `ExpressionLangString` value with no `expression` | [`EditError::TooFewReferences`] |
/// | an `expression` the element does not already hold that holds white space, names no element, or names an element that is no `Term` or `Expression` | [`EditError::InvalidReference`], [`EditError::ReferenceNotFound`], [`EditError::WrongReferenceType`], end `expression` (SACM23-BASE-009) |
/// | a freed id that a reference end names, then one that XMI metadata names | [`EditError::PartReference`], [`EditError::MetadataReference`] (LIB-EDIT-026) |
/// | last, a content holding a character XML 1.0 cannot represent | [`EditError::UnrepresentableCharacter`] |
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SetContent {
    element: ElementId,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    content: Option<MultiLangString>,
}

impl SetContent {
    /// Sets the content of `element` to `content`. A [`LangString`] converts
    /// into a content of one value; a `&str` or `String` converts too.
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

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_text(
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
/// - **Replaces the whole key** with a `MultiLangString`, held exactly, under the rules
///   of [`SetContent::new`]: an empty key is accepted and reported by
///   [`validation::validate`](crate::validation::validate), and ids are kept by position.
/// - **No remove.** A key is required, so there is no form that removes it.
/// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind that
///   is no `TaggedValue` is [`EditError::FeatureNotApplicable`]. Then it checks its
///   `ExpressionLangString` values, the ids it frees, and last its text, as
///   [`SetContent`] does (SACM23-BASE-009).
/// - **Freed ids.** The ids of values beyond the new length are freed. It refuses to free
///   one that a reference end or XMI metadata names ([`EditError::PartReference`],
///   [`EditError::MetadataReference`]; LIB-EDIT-026).
/// - **Apply it** with [`edit::apply`](super::apply).
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
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

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_text(document, &self.element, TextRole::Key, Some(&self.key))
    }
}

/// Sets the `content` of the element's `TaggedValue` with a plain key
/// (SACM 2.3 §8.12), creating the `TaggedValue` when it has none.
///
/// - **The key rule.** The tagged value it sets is the one
///   [`SacmElement::tagged_value`](crate::model::SacmElement::tagged_value) finds: the
///   first whose key has a value with the content `key`, byte for byte.
/// - **None has the key:** it creates a `TaggedValue` as the element's last child, with
///   the key `MultiLangString::from(key)`, the content, and a generated id,
///   `tagged_value_<n>`, as [`CreateTaggedValue`](super::CreateTaggedValue) would.
/// - **What a create lists.** The element is not changed: the tagged value is in
///   [`created_ids`](super::Mutation::created_ids).
/// - **One has it:** it replaces that tagged value's whole content, keeping ids by
///   position as [`SetContent`] does.
/// - **What a set lists.** The tagged value is in
///   [`changed_ids`](super::Mutation::changed_ids), unless the content is the one held,
///   which changes nothing.
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// It refuses, in this order:
///
/// | Refused | Refusal |
/// |---|---|
/// | an unknown element | [`EditError::ElementNotFound`] |
/// | a kind that holds no tagged value (§8.6: a `UtilityElement`) | [`EditError::FeatureNotApplicable`], naming `taggedValue` |
/// | the content's `ExpressionLangString`s, as [`SetContent`] checks them; on the set path, an `expression` the tagged value already holds is kept | as [`SetContent`] |
/// | on the set path, the ids of values beyond the new length that a reference end, then XMI metadata, names | [`EditError::PartReference`], [`EditError::MetadataReference`] (LIB-EDIT-026) |
/// | last, a key or content holding a character XML 1.0 cannot represent | [`EditError::UnrepresentableCharacter`], naming the tagged value |
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetTaggedValue};
/// use sacm::model::{Document, ElementId, LangString, MultiLangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
///
/// // No tagged value has the key: one is created.
/// let made = edit::apply(&mut doc, SetTaggedValue::new("c1", "gsn:id", "G1"))?;
/// assert_eq!(made.created_id().map(ElementId::as_str), Some("tagged_value_1"));
///
/// // Now one has it: its content is set.
/// let set = edit::apply(&mut doc, SetTaggedValue::new("c1", "gsn:id", "G2"))?;
/// assert_eq!(set.changed_ids(), [ElementId::from("tagged_value_1")]);
/// let c1 = doc.get("c1").expect("in the document");
/// let content = c1.tagged_value("gsn:id").and_then(|tag| tag.content());
/// assert_eq!(content, Some(&MultiLangString::from("G2")));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SetTaggedValue {
    element: ElementId,
    key: String,
    content: MultiLangString,
}

impl SetTaggedValue {
    /// Sets the content of the tagged value of `element` with the key `key` to
    /// `content`, creating the tagged value if there is none.
    ///
    /// A `&str`, a `String` or a [`LangString`] converts into a content of one value.
    #[must_use]
    pub fn new(
        element: impl Into<ElementId>,
        key: impl Into<String>,
        content: impl Into<MultiLangString>,
    ) -> Self {
        Self {
            element: element.into(),
            key: key.into(),
            content: content.into(),
        }
    }

    /// The id of the element whose tagged value to set.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The plain key, matched against the content of each value of a
    /// tagged value's key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The content to set.
    #[must_use]
    pub fn content(&self) -> &MultiLangString {
        &self.content
    }

    /// Runs every check of the operation, and returns what it creates or
    /// changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        // 1 and 2: the element, of a kind that holds tagged values.
        let target = edited(document, &self.element, TAGGED_VALUE, |kind| {
            kind.may_contain(ElementKind::TaggedValue)
        })?;
        let matched = target.tagged_value(&self.key);
        // 3 to 6: the content's expressions; those the matched tagged value
        // holds are kept, and a new one holds none.
        let held = matched.map_or(&[][..], SacmElement::expressions);
        check_expressions(document, self.content.values(), held)?;
        let Some(tag) = matched else {
            return plan_create(
                document,
                ElementKind::TaggedValue,
                Some(&self.element),
                None,
                Features::Texts {
                    content: &self.content,
                    key: Some(Cow::Owned(MultiLangString::from(self.key.as_str()))),
                },
            );
        };
        let held = tag.text(TextRole::Content);
        let text = Text::keeping_ids(self.content.clone(), held);
        if held == Some(&text) {
            return Ok(Plan::Unchanged);
        }
        Ok(Plan::Set {
            element: Cow::Owned(tag.id().clone()),
            value: Value::Text(TextRole::Content, Some(text)),
        })
    }
}

/// Defines the operation that sets or removes one string-valued attribute
/// of an `ArtifactAsset` kind (D-DATE; ADR 0034 amendment 1): the struct
/// `$name`, holding the value in `$field`, its methods, and its conversion
/// into an [`Operation`](super::Operation).
macro_rules! string_set {
    (
        $(#[$meta:meta])*
        $name:ident, $field:ident, $attribute:ident, $what:literal
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(deny_unknown_fields)
        )]
        pub struct $name {
            element: ElementId,
            #[cfg_attr(
                feature = "serde",
                serde(deserialize_with = "serde::Deserialize::deserialize")
            )]
            $field: Option<String>,
        }

        impl $name {
            #[doc = concat!(
                "Sets the `", $what, "` of `element` to `", stringify!($field),
                "`, exactly as given."
            )]
            #[must_use]
            pub fn new(element: impl Into<ElementId>, $field: impl Into<String>) -> Self {
                Self {
                    element: element.into(),
                    $field: Some($field.into()),
                }
            }

            #[doc = concat!("Removes the `", $what, "` of `element`.")]
            #[must_use]
            pub fn remove(element: impl Into<ElementId>) -> Self {
                Self {
                    element: element.into(),
                    $field: None,
                }
            }

            /// The id of the element to edit.
            #[must_use]
            pub fn element(&self) -> &ElementId {
                &self.element
            }

            #[doc = concat!("The `", $what, "` to set, or `None` to remove it.")]
            #[must_use]
            pub fn $field(&self) -> Option<&str> {
                self.$field.as_deref()
            }

            /// Runs every check of the operation, and returns what it
            /// changes.
            pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
                plan_string(
                    document,
                    &self.element,
                    StringAttribute::$attribute,
                    self.$field.as_deref(),
                )
            }
        }

        impl From<$name> for super::Operation {
            fn from(operation: $name) -> Self {
                Self::$name(operation)
            }
        }
    };
}

string_set! {
    /// Sets or removes the `version` (SACM 2.3 §12.7 `version: String[0..1]`,
    /// "the version of the artifact") of an `Artifact`.
    ///
    /// - **[`new`](Self::new)** sets it exactly as given: `""` is a value, not a removal,
    ///   and nothing is trimmed. [`remove`](Self::remove) removes it.
    /// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind
    ///   that is no `Artifact` is [`EditError::FeatureNotApplicable`], naming `version`.
    /// - **Last,** a version holding a character XML 1.0 cannot represent is
    ///   [`EditError::UnrepresentableCharacter`].
    /// - **The element** is in [`changed_ids`](super::Mutation::changed_ids) when its
    ///   version changes.
    /// - **Apply it** with [`edit::apply`](super::apply).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifact, CreateArtifactPackage, SetVersion};
    /// use sacm::model::{Document, ElementId, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
    /// let report = CreateArtifact::new("evidence", en("Brake test report")).with_id("report");
    /// edit::apply(&mut doc, report.with_version("1.2"))?;
    ///
    /// let set = edit::apply(&mut doc, SetVersion::new("report", "1.3"))?;
    /// assert_eq!(set.changed_ids(), [ElementId::from("report")]);
    /// assert_eq!(doc.get("report").and_then(|report| report.version()), Some("1.3"));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    SetVersion, version, Version, "version"
}

string_set! {
    /// Sets or removes the `date` of an `Artifact` or an `Event` (SACM 2.3 §12.7,
    /// §12.9).
    ///
    /// - **The date** is "the date on which the artifact was created" (§12.7), or "the
    ///   date on which the Event occurred" (§12.9).
    /// - **Any string.** SACM 2.3 gives `date` no lexical form, and its XML Schema types
    ///   it `xsd:string`. So [`new`](Self::new) holds any string exactly as given,
    ///   `2024-03-01`, `spring 2024` and `""` alike, and nothing ever reports one (D-DATE).
    /// - **[`remove`](Self::remove)** removes it.
    /// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind
    ///   that is neither an `Artifact` nor an `Event` is
    ///   [`EditError::FeatureNotApplicable`], naming `date`.
    /// - **Last,** a date holding a character XML 1.0 cannot represent is
    ///   [`EditError::UnrepresentableCharacter`].
    /// - **Apply it** with [`edit::apply`](super::apply).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifactPackage, CreateEvent, SetDate};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
    /// edit::apply(&mut doc, CreateEvent::new("evidence", en("Report released")).with_id("released"))?;
    ///
    /// edit::apply(&mut doc, SetDate::new("released", "2024-03-05T14:00:00+01:00"))?;
    /// let date = doc.get("released").and_then(|event| event.date());
    /// assert_eq!(date, Some("2024-03-05T14:00:00+01:00"));
    /// edit::apply(&mut doc, SetDate::remove("released"))?;
    /// assert_eq!(doc.get("released").and_then(|event| event.date()), None);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    SetDate, date, Date, "date"
}

string_set! {
    /// Sets or removes the `startTime` (SACM 2.3 §12.11, "time when the
    /// activity started") of an `Activity`.
    ///
    /// - **Held exactly as given,** as [`SetDate`]'s value is (D-DATE).
    /// - **No order is checked** against the `endTime`: SACM 2.3 states none.
    /// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind
    ///   that is no `Activity` is [`EditError::FeatureNotApplicable`], naming `startTime`.
    /// - **Last,** a value holding a character XML 1.0 cannot represent is
    ///   [`EditError::UnrepresentableCharacter`], naming
    ///   [`WriteAttribute::StartTime`](crate::xmi::WriteAttribute::StartTime), as
    ///   [`xmi::write`](crate::xmi::write) would.
    /// - **Apply it** with [`edit::apply`](super::apply).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateActivity, CreateArtifactPackage, SetStartTime};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
    /// edit::apply(&mut doc, CreateActivity::new("evidence", en("Brake testing")).with_id("testing"))?;
    /// edit::apply(&mut doc, SetStartTime::new("testing", "2024-02-12"))?;
    /// assert_eq!(doc.get("testing").and_then(|testing| testing.start_time()), Some("2024-02-12"));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    SetStartTime, start_time, StartTime, "startTime"
}

string_set! {
    /// Sets or removes the `endTime` (SACM 2.3 §12.11, "time when the
    /// activity ended") of an `Activity`.
    ///
    /// - **Held exactly as given,** as [`SetDate`]'s value is (D-DATE).
    /// - **No order is checked** against the `startTime`: SACM 2.3 states none.
    /// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind
    ///   that is no `Activity` is [`EditError::FeatureNotApplicable`], naming `endTime`.
    /// - **Last,** a value holding a character XML 1.0 cannot represent is
    ///   [`EditError::UnrepresentableCharacter`], naming
    ///   [`WriteAttribute::EndTime`](crate::xmi::WriteAttribute::EndTime), as
    ///   [`xmi::write`](crate::xmi::write) would.
    /// - **Apply it** with [`edit::apply`](super::apply).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateActivity, CreateArtifactPackage, SetEndTime};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
    /// let testing = CreateActivity::new("evidence", en("Brake testing")).with_id("testing");
    /// edit::apply(&mut doc, testing.with_end_time("2024-02-29T17:30:00Z"))?;
    /// edit::apply(&mut doc, SetEndTime::remove("testing"))?;
    /// assert_eq!(doc.get("testing").and_then(|testing| testing.end_time()), None);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    SetEndTime, end_time, EndTime, "endTime"
}

/// Sets the `value` (SACM 2.3 §10.9 `value:String[1]`, "the value of the
/// expression") of a `Term` or an `Expression`.
///
/// - **Exactly as given:** `""` is a value, and nothing is trimmed.
/// - **No remove.** A value is required, so there is no form that removes it, as for
///   [`SetKey`]. A file without one still reads, and
///   [`validation::validate`](crate::validation::validate) reports it.
/// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind that
///   is no `ExpressionElement` is [`EditError::FeatureNotApplicable`], naming `value`.
/// - **Last,** a value holding a character XML 1.0 cannot represent is
///   [`EditError::UnrepresentableCharacter`].
/// - **The element** is in [`changed_ids`](super::Mutation::changed_ids) exactly when
///   its value changes.
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateTerm, CreateTerminologyPackage, SetValue};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateTerminologyPackage::root(en("Terms")).with_id("terms"))?;
/// edit::apply(&mut doc, CreateTerm::new("terms", en("Blade"), "blade").with_id("blade"))?;
/// edit::apply(&mut doc, SetValue::new("blade", "rotor blade"))?;
/// assert_eq!(doc.get("blade").and_then(|blade| blade.value()), Some("rotor blade"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SetValue {
    element: ElementId,
    value: String,
}

impl SetValue {
    /// Sets the value of `element` to `value`, exactly as given.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, value: impl Into<String>) -> Self {
        Self {
            element: element.into(),
            value: value.into(),
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The value to set.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_string(
            document,
            &self.element,
            StringAttribute::Value,
            Some(&self.value),
        )
    }
}

impl From<SetValue> for super::Operation {
    fn from(operation: SetValue) -> Self {
        Self::SetValue(operation)
    }
}

string_set! {
    /// Sets or removes the `externalReference` (SACM 2.3 §10.11
    /// `externalReference:String[0..1]`) of a `Term`.
    ///
    /// - **What it is.** "An external reference (e.g., URI) to the object referred to by
    ///   the Term" (§10.11).
    /// - **[`new`](Self::new)** sets it exactly as given: `""` is a value, not a removal,
    ///   and nothing is trimmed or checked as a URI. [`remove`](Self::remove) removes it.
    /// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind
    ///   that is no `Term` is [`EditError::FeatureNotApplicable`], naming
    ///   `externalReference`.
    /// - **Last,** one holding a character XML 1.0 cannot represent is
    ///   [`EditError::UnrepresentableCharacter`].
    /// - **Apply it** with [`edit::apply`](super::apply).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateTerm, CreateTerminologyPackage, SetExternalReference};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateTerminologyPackage::root(en("Terms")).with_id("terms"))?;
    /// edit::apply(&mut doc, CreateTerm::new("terms", en("Blade"), "blade").with_id("blade"))?;
    /// edit::apply(&mut doc, SetExternalReference::new("blade", "https://example.org/terms/blade"))?;
    /// let reference = doc.get("blade").and_then(|blade| blade.external_reference());
    /// assert_eq!(reference, Some("https://example.org/terms/blade"));
    /// edit::apply(&mut doc, SetExternalReference::remove("blade"))?;
    /// assert_eq!(doc.get("blade").and_then(|blade| blade.external_reference()), None);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    SetExternalReference, external_reference, ExternalReference, "externalReference"
}

/// Sets or removes the `location` (SACM 2.3 §12.10 `location:Base::MultiLangString`) of
/// a `Resource`.
///
/// - **What it is.** "The path or URL specifying the location of the Resource, can be in
///   multiple languages" (§12.10).
/// - **Rules.** Those of [`SetContent`].
/// - **[`new`](Self::new)** replaces the whole location with a `MultiLangString`, held
///   exactly. An empty one, or one with a repeated language, is accepted, and
///   [`validation::validate`](crate::validation::validate) reports it (SACM23-VAL-010).
/// - **[`remove`](Self::remove)** removes it.
/// - **Ids are kept by position.** A replaced location keeps the `xmi:id` of its
///   `location` element, and each value keeps the id of the value at its position.
/// - **Freed ids.** The ids of values beyond the new length are freed, as are all of
///   them on a removal. It refuses to free one that a reference end or XMI metadata names
///   ([`EditError::PartReference`], [`EditError::MetadataReference`]; LIB-EDIT-026).
/// - **Refusals.** An unknown element is [`EditError::ElementNotFound`]; then a kind that
///   is no `Resource` is [`EditError::FeatureNotApplicable`], naming `location`. Then it
///   checks its `ExpressionLangString` values as [`SetContent`] does (SACM23-BASE-009).
/// - **Last,** a location holding a character XML 1.0 cannot represent is
///   [`EditError::UnrepresentableCharacter`].
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArtifactPackage, CreateResource, SetLocation};
/// use sacm::model::{Document, LangString, MultiLangString};
///
/// let mut doc = Document::new();
/// let lang = |text: &str, lang: &str| LangString::new(text).with_lang(lang);
/// edit::apply(&mut doc, CreateArtifactPackage::root(lang("Evidence", "en")).with_id("evidence"))?;
/// edit::apply(&mut doc, CreateResource::new("evidence", lang("Report PDF", "en")).with_id("pdf"))?;
///
/// let location: MultiLangString = [
///     lang("https://example.org/reports/brake.pdf", "en"),
///     lang("https://example.org/rapporter/brake.pdf", "sv"),
/// ]
/// .into_iter()
/// .collect();
/// edit::apply(&mut doc, SetLocation::new("pdf", location.clone()))?;
/// assert_eq!(doc.get("pdf").and_then(|pdf| pdf.location()), Some(&location));
/// edit::apply(&mut doc, SetLocation::remove("pdf"))?;
/// assert_eq!(doc.get("pdf").and_then(|pdf| pdf.location()), None);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SetLocation {
    element: ElementId,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    location: Option<MultiLangString>,
}

impl SetLocation {
    /// Sets the location of `element` to `location`. A [`LangString`]
    /// converts into a location of one value.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, location: impl Into<MultiLangString>) -> Self {
        Self {
            element: element.into(),
            location: Some(location.into()),
        }
    }

    /// Removes the location of `element`.
    #[must_use]
    pub fn remove(element: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            location: None,
        }
    }

    /// The id of the element to edit.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The location to set, or `None` to remove it.
    #[must_use]
    pub fn location(&self) -> Option<&MultiLangString> {
        self.location.as_ref()
    }

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_text(
            document,
            &self.element,
            TextRole::Location,
            self.location.as_ref(),
        )
    }
}

/// The plan of setting the string-valued `attribute` of `element` to
/// `value`, or of removing it when `None`: a change exactly when the value
/// changes.
fn plan_string<'o>(
    document: &Document,
    element: &'o ElementId,
    attribute: StringAttribute,
    value: Option<&'o str>,
) -> Result<Plan<'o>, EditError> {
    let target = edited(document, element, attribute.name(), |kind| {
        attribute.is_held_by(kind)
    })?;
    Ok(changing(
        element,
        target.string(attribute) != value,
        Value::String(attribute, value),
    ))
}

/// The plan of setting the `role` of `element` to `value`, or of removing
/// it when `None`, keeping the ids by position: a change exactly when the
/// value or its ids change.
fn plan_text<'o>(
    document: &Document,
    element: &'o ElementId,
    role: TextRole,
    value: Option<&MultiLangString>,
) -> Result<Plan<'o>, EditError> {
    let target = edited(document, element, role.name(), |kind| role.is_held_by(kind))?;
    if let Some(value) = value {
        check_expressions(document, value.values(), target.expressions())?;
    }
    let held = target.text(role);
    let text = value.map(|value| Text::keeping_ids(value.clone(), held));
    let changes = held != text.as_ref();
    Ok(changing(element, changes, Value::Text(role, text)))
}

/// The plan of an operation that sets a feature of `element` to `value`:
/// [`Plan::Unchanged`] unless the value `changes`.
pub(super) fn changing<'o>(element: &'o ElementId, changes: bool, value: Value<'o>) -> Plan<'o> {
    if changes {
        Plan::Set {
            element: Cow::Borrowed(element),
            value,
        }
    } else {
        Plan::Unchanged
    }
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
