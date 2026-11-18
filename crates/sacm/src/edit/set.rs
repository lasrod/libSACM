//! The operations that set a feature of an existing element: its name, gid,
//! flags, `assertionDeclaration`, a `UtilityElement`'s `content` and `key`
//! (LIB-EDIT-008, LIB-EDIT-012, ADR 0029), and an `ArtifactAsset`'s
//! `version`, `date`, `location`, `startTime` and `endTime` (SACM23-ART-002,
//! SACM23-ART-004, SACM23-ART-005; ADR 0034 amendment 1).

use super::EditError;
use super::plan::{Plan, Value};
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
/// name, as [`EditError::FeatureNotApplicable`]. A name that is an
/// `ExpressionLangString` (§8.4) must have an `expression`
/// ([`EditError::TooFewReferences`]), and an `expression` the element does
/// not already hold must name a `Term` or an `Expression` of the document
/// ([`EditError::InvalidReference`], [`EditError::ReferenceNotFound`],
/// [`EditError::WrongReferenceType`], end `expression`; SACM23-BASE-009);
/// [`SetContent`], [`SetKey`] and [`SetLocation`] check their values the
/// same way. The element is in
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

/// Sets or removes the `content` (SACM 2.3 §8.7
/// `content:MultiLangString[0..1]`) of a `UtilityElement`: a Description,
/// `ImplementationConstraint`, Note or `TaggedValue`.
///
/// [`new`](Self::new) replaces the whole content with any
/// `MultiLangString` it does not refuse below, held exactly: an empty one,
/// or one with a repeated
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
/// as [`EditError::FeatureNotApplicable`]. Then it refuses an
/// `ExpressionLangString` value with no `expression` as
/// [`EditError::TooFewReferences`], and an `expression` the element does not
/// already hold that holds white space, names no element, or names an
/// element that is no `Term` or `Expression` ([`EditError::InvalidReference`],
/// [`EditError::ReferenceNotFound`], [`EditError::WrongReferenceType`], end
/// `expression`; SACM23-BASE-009). The `UtilityElement`, not its
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
/// It replaces the whole key with a `MultiLangString`, held exactly, under
/// the rules of [`SetContent::new`]: an empty key is accepted and reported
/// by [`validation::validate`](crate::validation::validate), and ids are
/// kept by position. A key is required, so there is no form that removes
/// it. Apply it with [`edit::apply`](super::apply). It refuses an unknown
/// element as [`EditError::ElementNotFound`], then a kind that is no
/// `TaggedValue` as [`EditError::FeatureNotApplicable`]. Then it checks its
/// `ExpressionLangString` values as [`SetContent`] does (SACM23-BASE-009).
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
    /// [`new`](Self::new) sets it exactly as given: `""` is a value, not a
    /// removal, and nothing is trimmed. [`remove`](Self::remove) removes it.
    /// A version holding a character XML cannot represent is held, and
    /// [`xmi::write`](crate::xmi::write) refuses it
    /// ([`WriteAttribute::Version`](crate::xmi::WriteAttribute::Version)).
    /// Apply it with [`edit::apply`](super::apply). It refuses an unknown
    /// element as [`EditError::ElementNotFound`], then a kind that is no
    /// `Artifact` as [`EditError::FeatureNotApplicable`], naming `version`.
    /// The element is in [`changed_ids`](super::Mutation::changed_ids) when
    /// its version changes.
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
    /// Sets or removes the `date` of an `Artifact` (SACM 2.3 §12.7, "the
    /// date on which the artifact was created") or an `Event` (§12.9, "the
    /// date on which the Event occurred").
    ///
    /// SACM 2.3 gives `date` no lexical form, and its XML Schema types it
    /// `xsd:string`, so [`new`](Self::new) holds any string exactly as given,
    /// `2024-03-01`, `spring 2024` and `""` alike, and nothing ever reports
    /// one (D-DATE). [`remove`](Self::remove) removes it. A date holding a
    /// character XML cannot represent is held, and
    /// [`xmi::write`](crate::xmi::write) refuses it
    /// ([`WriteAttribute::Date`](crate::xmi::WriteAttribute::Date)). Apply it
    /// with [`edit::apply`](super::apply). It refuses an unknown element as
    /// [`EditError::ElementNotFound`], then a kind that is neither an
    /// `Artifact` nor an `Event` as [`EditError::FeatureNotApplicable`],
    /// naming `date`.
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
    /// The value is held exactly as given, as [`SetDate`]'s is (D-DATE), and
    /// nothing checks it against the `endTime`: SACM 2.3 states no order.
    /// Apply it with [`edit::apply`](super::apply). It refuses an unknown
    /// element as [`EditError::ElementNotFound`], then a kind that is no
    /// `Activity` as [`EditError::FeatureNotApplicable`], naming `startTime`;
    /// [`xmi::write`](crate::xmi::write) refuses a character XML cannot
    /// represent
    /// ([`WriteAttribute::StartTime`](crate::xmi::WriteAttribute::StartTime)).
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
    /// The value is held exactly as given, as [`SetDate`]'s is (D-DATE), and
    /// nothing checks it against the `startTime`: SACM 2.3 states no order.
    /// Apply it with [`edit::apply`](super::apply). It refuses an unknown
    /// element as [`EditError::ElementNotFound`], then a kind that is no
    /// `Activity` as [`EditError::FeatureNotApplicable`], naming `endTime`;
    /// [`xmi::write`](crate::xmi::write) refuses a character XML cannot
    /// represent
    /// ([`WriteAttribute::EndTime`](crate::xmi::WriteAttribute::EndTime)).
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
/// It sets it exactly as given: `""` is a value, and nothing is trimmed. A
/// value is required, so there is no form that removes it, as for
/// [`SetKey`]; a file without one still reads, and
/// [`validation::validate`](crate::validation::validate) reports it. A value
/// holding a character XML cannot represent is held, and
/// [`xmi::write`](crate::xmi::write) refuses it
/// ([`WriteAttribute::Value`](crate::xmi::WriteAttribute::Value)). Apply it
/// with [`edit::apply`](super::apply). It refuses an unknown element as
/// [`EditError::ElementNotFound`], then a kind that is no `ExpressionElement`
/// as [`EditError::FeatureNotApplicable`], naming `value`. The element is in
/// [`changed_ids`](super::Mutation::changed_ids) exactly when its value
/// changes.
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
    /// `externalReference:String[0..1]`, "an external reference (e.g., URI)
    /// to the object referred to by the Term") of a `Term`.
    ///
    /// [`new`](Self::new) sets it exactly as given: `""` is a value, not a
    /// removal, and nothing is trimmed or checked as a URI.
    /// [`remove`](Self::remove) removes it. One holding a character XML
    /// cannot represent is held, and [`xmi::write`](crate::xmi::write)
    /// refuses it
    /// ([`WriteAttribute::ExternalReference`](crate::xmi::WriteAttribute::ExternalReference)).
    /// Apply it with [`edit::apply`](super::apply). It refuses an unknown
    /// element as [`EditError::ElementNotFound`], then a kind that is no
    /// `Term` as [`EditError::FeatureNotApplicable`], naming
    /// `externalReference`.
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

/// Sets or removes the `location` (SACM 2.3 §12.10
/// `location:Base::MultiLangString`, "the path or URL specifying the location
/// of the Resource, can be in multiple languages") of a `Resource`.
///
/// It follows the rules of [`SetContent`]: [`new`](Self::new) replaces the
/// whole location with a `MultiLangString`, held exactly, an empty one or
/// one with a repeated language included, which
/// [`validation::validate`](crate::validation::validate) reports
/// (SACM23-VAL-010); [`remove`](Self::remove) removes it. A replaced
/// location keeps the `xmi:id` of its `location` element, and each value
/// keeps the id of the value at its position; the ids of values beyond the
/// new length are freed, as are all of them on a removal. A location
/// holding a character XML cannot represent is held, and
/// [`xmi::write`](crate::xmi::write) refuses it. Apply it with
/// [`edit::apply`](super::apply). It refuses an unknown element as
/// [`EditError::ElementNotFound`], then a kind that is no `Resource` as
/// [`EditError::FeatureNotApplicable`], naming `location`. Then it checks
/// its `ExpressionLangString` values as [`SetContent`] does (SACM23-BASE-009).
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
        Plan::Set { element, value }
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
