//! The `LangString` value type (formal/23-05-08 §8.3).

/// A string with an optional language tag: SACM's `LangString` (§8.3).
///
/// §8.3 gives it two attributes, `lang : String [0..1]` ("the language used
/// in the string") and `content : String [0..1]` ("the content of the
/// string").
///
/// For each field, absent (`None`), empty (`Some("")`) and non-empty are
/// three distinct values (ADR 0012), and [`Eq`], [`Ord`] and [`Hash`] tell
/// them apart. Values are kept byte for byte: there is no trimming, case
/// folding or Unicode normalisation, and `lang` has no format check.
///
/// `LangString` is a value, not an element: it has no id and no
/// [`ElementKind`](crate::model::ElementKind). You can build one freely.
///
/// [`Ord`] compares `lang` first, then `content`, with absent before present.
///
/// # Examples
///
/// ```
/// use sacm::model::LangString;
///
/// let name = LangString::new("The system is acceptably safe").with_lang("en");
/// assert_eq!(name.lang(), Some("en"));
/// assert_eq!(name.content(), Some("The system is acceptably safe"));
///
/// // Absent and empty are different values.
/// assert_eq!(LangString::default().content(), None);
/// assert_eq!(LangString::new("").content(), Some(""));
/// assert_ne!(LangString::new(""), LangString::default());
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
