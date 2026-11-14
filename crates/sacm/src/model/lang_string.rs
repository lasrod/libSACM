//! The `LangString` and `MultiLangString` value types (formal/23-05-08 §8.3,
//! §8.5).

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
/// [`Ord`] compares `lang` first, then `content`, with absent before present.
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
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LangString {
    // Field order is the `Ord` order: `lang`, then `content`.
    lang: Option<String>,
    content: Option<String>,
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
