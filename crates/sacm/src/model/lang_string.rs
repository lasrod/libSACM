//! The `LangString` and `MultiLangString` value types (formal/23-05-08 §8.3,
//! §8.4, §8.5), and the `ExpressionLangString` form of a `LangString`
//! (SACM23-BASE-009, ADR 0038).

use std::fmt;

use super::{ElementId, Href};

/// Text with an optional language tag, where the text may also be absent:
/// SACM's `LangString` (SACM 2.3 §8.3).
///
/// SACM uses it for the `name` of every `ModelElement` (SACM 2.3 §8.6), read
/// with [`SacmElement::name`](crate::model::SacmElement::name), and for each
/// entry of a [`MultiLangString`] (§8.5). SACM defines no format for `lang`,
/// so any string is accepted.
///
/// §8.3 gives it two attributes, `lang : String [0..1]` ("the language used
/// in the string") and `content : String [0..1]` ("the content of the
/// string").
///
/// For each field, absent (`None`), empty (`Some("")`) and non-empty are
/// three distinct values ([ADR 0012]), and [`Eq`], [`Ord`] and [`Hash`] tell
/// them apart. Values are kept byte for byte: there is no trimming, case
/// folding or Unicode normalisation, and `lang` has no format check.
///
/// [ADR 0012]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0012-lang-string-absent-and-empty.md
///
/// `LangString` is a value, not an element: it has no id and no
/// [`ElementKind`](crate::model::ElementKind), and you construct it directly,
/// unlike elements, which only a [`Document`](crate::model::Document) holds.
/// A `name` element may still carry an `xmi:id` (a `LangString` is a class,
/// XMI 2.5.1 §7.8.3); the document keeps it with the element that owns the
/// name, as [`SacmElement::name_id`](crate::model::SacmElement::name_id), not
/// on the value.
///
/// [`Ord`] compares `lang` first, then `content`, with absent before present,
/// then the form: plain before an `ExpressionLangString`, and among those
/// one with no expression, then one with an id, then one with an href.
///
/// # `ExpressionLangString`
///
/// A `LangString` may also be an `ExpressionLangString` (§8.4: "it contains a
/// description (`LangString`) and it also (optionally) points to an
/// `ExpressionElement` in the Terminology Package"): its
/// [`expression`](Self::expression) is the id of that `Term` or
/// `Expression`. [`with_expression`](Self::with_expression) makes one and
/// [`without_expression`](Self::without_expression) makes it plain again,
/// keeping `lang` and `content` (ADR 0038). §8.4 gives `expression[1]`, but
/// a file may hold one without, which reads with `expression() == None` and
/// which [`validate`](crate::validation::validate) reports (SACM-MULT-001).
/// The ids an element's name and texts link are its
/// [`expressions`](crate::model::SacmElement::expressions). The expression
/// may instead be an element of another document, named by an
/// [`Href`]: [`expression_href`](Self::expression_href) and
/// [`with_expression_href`](Self::with_expression_href) (ADR 0040 amendment
/// 1). It then counts as present, and is never resolved or checked.
///
/// ```
/// use sacm::model::{ElementId, LangString};
///
/// let name = LangString::default().with_lang("en").with_expression("blade_guarded");
/// assert!(name.is_expression_lang_string());
/// assert_eq!(name.expression(), Some(&ElementId::from("blade_guarded")));
/// assert_eq!(name.clone().without_expression(), LangString::default().with_lang("en"));
/// assert!(LangString::default().with_lang("en") < name);
/// ```
///
/// # Examples
///
/// ```
/// use sacm::model::LangString;
///
/// let name = LangString::new("C1").with_lang("en");
/// assert_eq!(name.lang(), Some("en"));
/// assert_eq!(name.content(), Some("C1"));
///
/// // Absent and empty are different values.
/// assert_eq!(LangString::default().content(), None);
/// assert_eq!(LangString::new("").content(), Some(""));
/// assert_ne!(LangString::new(""), LangString::default());
///
/// // A lang-only value has no content.
/// let lang_only = LangString::default().with_lang("en");
/// assert_eq!(lang_only.content(), None);
/// ```
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LangString {
    // Field order is the `Ord` order: `lang`, then `content`, then the form.
    lang: Option<String>,
    content: Option<String>,
    form: Form,
}

/// Whether a [`LangString`] is plain or an `ExpressionLangString` (§8.4),
/// with its `expression` if it has one. Plain sorts first.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Form {
    #[default]
    Plain,
    Expression(Option<Link>),
}

/// What the `expression` of an `ExpressionLangString` names (ADR 0040
/// amendment 1, A2): an element of this document by its id, or one of
/// another document by an href. An id sorts before an href.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Link {
    Id(ElementId),
    Href(Href),
}

impl LangString {
    /// A `LangString` with this content and no `lang`.
    ///
    /// ```
    /// use sacm::model::LangString;
    ///
    /// let value = LangString::new("Top claim");
    /// assert_eq!(value.lang(), None);
    /// assert_eq!(value.content(), Some("Top claim"));
    /// ```
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            lang: None,
            content: Some(content.into()),
            form: Form::Plain,
        }
    }

    /// This value with `lang` set to `lang`, replacing any earlier one.
    ///
    /// ```
    /// use sacm::model::LangString;
    ///
    /// let value = LangString::new("Säkerhetsfall").with_lang("sv");
    /// assert_eq!(value.lang(), Some("sv"));
    /// ```
    #[must_use]
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }

    /// The language tag, or `None` if it is absent.
    #[must_use]
    pub fn lang(&self) -> Option<&str> {
        self.lang.as_deref()
    }

    /// The content, or `None` if it is absent.
    ///
    /// Not to be confused with
    /// [`SacmElement::content`](crate::model::SacmElement::content), which is
    /// a `UtilityElement`'s whole `MultiLangString`.
    #[must_use]
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    /// Sets the language tag. `None` makes it absent.
    ///
    /// ```
    /// use sacm::model::LangString;
    ///
    /// let mut value = LangString::new("c").with_lang("en");
    /// value.set_lang(None);
    /// assert_eq!(value, LangString::new("c"));
    /// ```
    pub fn set_lang(&mut self, lang: Option<String>) {
        self.lang = lang;
    }

    /// Sets the content. `None` makes it absent.
    ///
    /// ```
    /// use sacm::model::LangString;
    ///
    /// let mut value = LangString::new("c");
    /// value.set_content(Some(String::new()));
    /// assert_eq!(value.content(), Some(""));
    /// ```
    pub fn set_content(&mut self, content: Option<String>) {
        self.content = content;
    }

    /// The `expression` of an `ExpressionLangString` (SACM 2.3 §8.4): the id of
    /// the `ExpressionElement` it links to. `None` for a plain `LangString`,
    /// for an `ExpressionLangString` read without one, and for one whose
    /// expression is in another document
    /// ([`expression_href`](Self::expression_href)).
    ///
    /// ```
    /// use sacm::model::{ElementId, LangString};
    ///
    /// assert_eq!(LangString::new("x").expression(), None);
    /// let linked = LangString::new("x").with_expression("blade");
    /// assert_eq!(linked.expression(), Some(&ElementId::from("blade")));
    /// ```
    #[must_use]
    pub fn expression(&self) -> Option<&ElementId> {
        match &self.form {
            Form::Expression(Some(Link::Id(expression))) => Some(expression),
            Form::Expression(_) | Form::Plain => None,
        }
    }

    /// The `expression` of an `ExpressionLangString` that is an element of
    /// another document, or an `XPointer`: the [`Href`] that names it, never
    /// resolved or checked (SACM23-XMI-029, ADR 0040 amendment 1). `None`
    /// for a plain `LangString`, and for an `ExpressionLangString` whose
    /// expression is an id or absent.
    ///
    /// It counts as the expression being present (§8.4 `expression[1]`),
    /// and it is not among the ids of
    /// [`SacmElement::expressions`](crate::model::SacmElement::expressions).
    ///
    /// ```
    /// use sacm::model::{Href, LangString};
    ///
    /// let href = Href::new("terms.sacm.xmi#blade")?;
    /// let name = LangString::new("x").with_expression_href(href.clone());
    /// assert_eq!(name.expression_href(), Some(&href));
    /// assert_eq!(name.expression(), None);
    /// assert_eq!(LangString::new("x").with_expression("blade").expression_href(), None);
    /// # Ok::<(), sacm::model::HrefError>(())
    /// ```
    #[must_use]
    pub fn expression_href(&self) -> Option<&Href> {
        match &self.form {
            Form::Expression(Some(Link::Href(href))) => Some(href),
            Form::Expression(_) | Form::Plain => None,
        }
    }

    /// Whether this is an `ExpressionLangString` with no `expression`,
    /// neither an id nor an href (§8.4 `expression[1]`).
    pub(crate) fn lacks_expression(&self) -> bool {
        matches!(self.form, Form::Expression(None))
    }

    /// Whether this is an `ExpressionLangString` (§8.4), with or without an
    /// `expression`.
    ///
    /// ```
    /// use sacm::model::LangString;
    ///
    /// assert!(!LangString::new("x").is_expression_lang_string());
    /// assert!(LangString::new("x").with_expression("blade").is_expression_lang_string());
    /// ```
    #[must_use]
    pub fn is_expression_lang_string(&self) -> bool {
        matches!(self.form, Form::Expression(_))
    }

    /// This value as an `ExpressionLangString` whose `expression` is
    /// `expression`, keeping `lang` and `content` and replacing any earlier
    /// expression.
    ///
    /// An edit that sets it checks the id as a reference: it must name a
    /// `Term` or an `Expression` of the document
    /// ([`EditError::ReferenceNotFound`](crate::edit::EditError::ReferenceNotFound),
    /// [`EditError::WrongReferenceType`](crate::edit::EditError::WrongReferenceType)).
    ///
    /// ```
    /// use sacm::model::{ElementId, LangString};
    ///
    /// let name = LangString::new("x").with_expression("blade").with_expression("lid");
    /// assert_eq!(name.expression(), Some(&ElementId::from("lid")));
    /// assert_eq!(name.content(), Some("x"));
    /// ```
    #[must_use]
    pub fn with_expression(mut self, expression: impl Into<ElementId>) -> Self {
        self.form = Form::Expression(Some(Link::Id(expression.into())));
        self
    }

    /// This value as an `ExpressionLangString` whose `expression` is the
    /// element of another document that `href` names, keeping `lang` and
    /// `content` and replacing any earlier expression (LIB-EDIT-020, ADR
    /// 0040 amendment 1).
    ///
    /// No edit checks the href: the element it names need not exist.
    ///
    /// ```
    /// use sacm::model::{ElementId, Href, LangString};
    ///
    /// let href = Href::new("terms.sacm.xmi#blade")?;
    /// let name = LangString::new("x").with_lang("en").with_expression_href(href.clone());
    /// assert!(name.is_expression_lang_string());
    /// assert_eq!(name.expression_href(), Some(&href));
    /// assert_eq!((name.lang(), name.content()), (Some("en"), Some("x")));
    /// // An id replaces it.
    /// let local = name.with_expression("blade");
    /// assert_eq!(local.expression(), Some(&ElementId::from("blade")));
    /// assert_eq!(local.expression_href(), None);
    /// # Ok::<(), sacm::model::HrefError>(())
    /// ```
    #[must_use]
    pub fn with_expression_href(mut self, href: Href) -> Self {
        self.form = Form::Expression(Some(Link::Href(href)));
        self
    }

    /// This value as a plain `LangString`, keeping `lang` and `content`.
    ///
    /// ```
    /// use sacm::model::LangString;
    ///
    /// let name = LangString::new("x").with_lang("en").with_expression("blade");
    /// assert_eq!(name.without_expression(), LangString::new("x").with_lang("en"));
    /// ```
    #[must_use]
    pub fn without_expression(mut self) -> Self {
        self.form = Form::Plain;
        self
    }

    /// This value as an `ExpressionLangString` with no `expression`, as a file
    /// may give one.
    pub(crate) fn with_no_expression(mut self) -> Self {
        self.form = Form::Expression(None);
        self
    }

    /// This value with its `expression` replaced by `map(expression)`, if it
    /// has one: a copy's link to a copied element.
    pub(crate) fn map_expression(&self, map: impl FnOnce(&ElementId) -> Option<ElementId>) -> Self {
        let mut value = self.clone();
        if let Form::Expression(Some(Link::Id(expression))) = &self.form {
            if let Some(mapped) = map(expression) {
                value.form = Form::Expression(Some(Link::Id(mapped)));
            }
        }
        value
    }
}

impl fmt::Debug for LangString {
    /// A plain value as `LangString { lang, content }`; an
    /// `ExpressionLangString` as `ExpressionLangString { lang, content,
    /// expression }`, the expression as its id, its href or `None`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.form {
            Form::Plain => f
                .debug_struct("LangString")
                .field("lang", &self.lang)
                .field("content", &self.content)
                .finish(),
            Form::Expression(expression) => {
                let mut out = f.debug_struct("ExpressionLangString");
                out.field("lang", &self.lang)
                    .field("content", &self.content);
                match expression {
                    Some(Link::Id(id)) => out.field("expression", &Some(id)),
                    Some(Link::Href(href)) => out.field("expression", &Some(href)),
                    None => out.field("expression", &None::<ElementId>),
                };
                out.finish()
            }
        }
    }
}

/// The class name a JSON `ExpressionLangString` is tagged with (ADR 0030
/// amendment 4).
#[cfg(feature = "serde")]
const EXPRESSION_LANG_STRING: &str = "ExpressionLangString";

/// The JSON of a `LangString` (ADR 0030 R6 and amendment 4): a plain one is
/// `{"lang", "content"}`; an `ExpressionLangString` is
/// `{"type": "ExpressionLangString", "lang", "content", "expression"}`, keys
/// in that order, `expression` the id as a string, `{"href": "…"}` for an
/// element of another document (ADR 0040 amendment 1, A7), or `null` when
/// it has none.
#[cfg(feature = "serde")]
impl serde::Serialize for LangString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        match &self.form {
            Form::Plain => {
                let mut out = serializer.serialize_struct("LangString", 2)?;
                out.serialize_field("lang", &self.lang)?;
                out.serialize_field("content", &self.content)?;
                out.end()
            }
            Form::Expression(expression) => {
                let mut out = serializer.serialize_struct("LangString", 4)?;
                out.serialize_field("type", EXPRESSION_LANG_STRING)?;
                out.serialize_field("lang", &self.lang)?;
                out.serialize_field("content", &self.content)?;
                match expression {
                    Some(Link::Id(id)) => out.serialize_field("expression", &Some(id))?,
                    Some(Link::Href(href)) => {
                        out.serialize_field("expression", &JsonHref { href: href.clone() })?;
                    }
                    None => out.serialize_field("expression", &None::<ElementId>)?,
                }
                out.end()
            }
        }
    }
}

/// The JSON of an expression that is an href: `{"href": "…"}`, and no other
/// key.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonHref {
    href: Href,
}

/// The JSON of an `expression`: an id string, `null`, or [`JsonHref`].
#[cfg(feature = "serde")]
struct JsonExpression(Option<Link>);

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for JsonExpression {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ExpressionVisitor;

        impl<'de> serde::de::Visitor<'de> for ExpressionVisitor {
            type Value = JsonExpression;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an id, null, or {\"href\": \"…\"}")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<JsonExpression, E> {
                Ok(JsonExpression(None))
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<JsonExpression, E> {
                Ok(JsonExpression(Some(Link::Id(ElementId::new(id)))))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<JsonExpression, A::Error> {
                use serde::Deserialize;
                let json =
                    JsonHref::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(JsonExpression(Some(Link::Href(json.href))))
            }
        }

        deserializer.deserialize_any(ExpressionVisitor)
    }
}

/// Reads either JSON form (ADR 0030 amendment 4): `type`, when given, must
/// be `"ExpressionLangString"`, and `expression` is required with it and
/// refused without it; `lang` and `content` may be left out (R3); unknown
/// and repeated keys are refused (R4).
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for LangString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("LangString", JSON_FIELDS, LangStringVisitor)
    }
}

/// The keys of a JSON `LangString`.
#[cfg(feature = "serde")]
const JSON_FIELDS: &[&str] = &["type", "lang", "content", "expression"];

/// The visitor of [`LangString`]'s `Deserialize`.
#[cfg(feature = "serde")]
struct LangStringVisitor;

#[cfg(feature = "serde")]
impl<'de> serde::de::Visitor<'de> for LangStringVisitor {
    type Value = LangString;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a LangString or an ExpressionLangString")
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<LangString, A::Error> {
        use serde::de::Error;
        let mut tag: Option<String> = None;
        let mut lang: Option<Option<String>> = None;
        let mut content: Option<Option<String>> = None;
        let mut expression: Option<JsonExpression> = None;
        while let Some(key) = map.next_key::<String>()? {
            let repeated = match key.as_str() {
                "type" => tag.replace(map.next_value()?).is_some(),
                "lang" => lang.replace(map.next_value()?).is_some(),
                "content" => content.replace(map.next_value()?).is_some(),
                "expression" => expression.replace(map.next_value()?).is_some(),
                other => return Err(A::Error::unknown_field(other, JSON_FIELDS)),
            };
            if repeated {
                return Err(A::Error::custom(format_args!("duplicate field `{key}`")));
            }
        }
        let form = match (tag, expression) {
            (None, None) => Form::Plain,
            (Some(tag), _) if tag != EXPRESSION_LANG_STRING => {
                return Err(A::Error::invalid_value(
                    serde::de::Unexpected::Str(&tag),
                    &"\"ExpressionLangString\"",
                ));
            }
            (Some(_), Some(JsonExpression(expression))) => Form::Expression(expression),
            (Some(_), None) => return Err(A::Error::missing_field("expression")),
            (None, Some(_)) => {
                return Err(A::Error::custom(
                    "`expression` is given without `type`: only an ExpressionLangString has one",
                ));
            }
        };
        Ok(LangString {
            lang: lang.unwrap_or_default(),
            content: content.unwrap_or_default(),
            form,
        })
    }
}

/// A list of [`LangString`] values that say the same thing in different
/// languages: SACM's `MultiLangString` (SACM 2.3 §8.5).
///
/// §8.5 gives it `value : LangString [1..*]` and requires each value's `lang`
/// to be unique. This type enforces neither: it keeps the values in the order
/// given, including duplicates and repeated languages, and it may be empty,
/// so a document can be held as it was written.
/// [`validation::validate`](crate::validation::validate) reports an empty
/// one and each repeated `lang` held in a document (SACM-MULT-001).
///
/// Equality and [`Hash`] depend on order: `[en, fr]` and `[fr, en]` are
/// different values.
///
/// SACM uses it for the `content` of the `UtilityElement` kinds (§8.7), such
/// as a `Note`
/// ([`SacmElement::content`](crate::model::SacmElement::content)), and for the
/// `key` of a `TaggedValue` (§8.12). It is a value, not an element: it has no
/// id and no [`ElementKind`](crate::model::ElementKind). The `xmi:id` its
/// element may carry is kept with the element that holds it
/// ([`SacmElement::content_id`](crate::model::SacmElement::content_id)).
///
/// # Examples
///
/// ```
/// use sacm::model::{LangString, MultiLangString};
///
/// let mut text: MultiLangString = [LangString::new("Top claim").with_lang("en")]
///     .into_iter()
///     .collect();
/// text.push(LangString::new("Huvudpåstående").with_lang("sv"));
/// assert_eq!(text.len(), 2);
/// assert_eq!(text.values()[1].lang(), Some("sv"));
///
/// // An empty list is allowed in memory.
/// assert!(MultiLangString::default().is_empty());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct MultiLangString {
    values: Vec<LangString>,
}

impl MultiLangString {
    /// The values, in the order they were given.
    #[must_use]
    pub fn values(&self) -> &[LangString] {
        &self.values
    }

    /// The number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether there are no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Adds `value` at the end. It never replaces or merges a value, even one
    /// with the same `lang`.
    ///
    /// ```
    /// use sacm::model::{LangString, MultiLangString};
    ///
    /// let mut text = MultiLangString::default();
    /// text.push(LangString::new("a").with_lang("en"));
    /// text.push(LangString::new("b").with_lang("en"));
    /// assert_eq!(text.len(), 2);
    /// ```
    pub fn push(&mut self, value: LangString) {
        self.values.push(value);
    }
}

impl From<LangString> for MultiLangString {
    /// A `MultiLangString` of the one value `value`.
    ///
    /// ```
    /// use sacm::model::{LangString, MultiLangString};
    ///
    /// let text = MultiLangString::from(LangString::new("Reviewed").with_lang("en"));
    /// assert_eq!(text.values(), [LangString::new("Reviewed").with_lang("en")]);
    /// ```
    fn from(value: LangString) -> Self {
        Self {
            values: vec![value],
        }
    }
}

impl FromIterator<LangString> for MultiLangString {
    /// Collects the values in iteration order.
    fn from_iter<I: IntoIterator<Item = LangString>>(iter: I) -> Self {
        Self {
            values: iter.into_iter().collect(),
        }
    }
}
