//! Value types of the Argumentation package (formal/23-05-08 clause 11).

use std::fmt;
use std::str::FromStr;

/// The declared state of an Assertion: SACM's `AssertionDeclaration`
/// enumeration (SACM 2.3 §11.8).
///
/// Every Assertion (a Claim or an asserted relationship; see
/// [`AbstractClass::Assertion`](crate::model::AbstractClass::Assertion)) has
/// one, in its `assertionDeclaration` attribute (SACM 2.3 §11.10). The model
/// does not attach it to elements yet.
///
/// The variants are in the metamodel's literal order, and [`Ord`] follows it.
/// The default is [`Asserted`](Self::Asserted) (§11.8, §11.10). Conversion
/// to and from the literal spellings is exact and case-sensitive:
/// [`as_str`](Self::as_str) and [`Display`](fmt::Display) give the spelling,
/// and [`FromStr`] accepts only that spelling.
///
/// It is `#[non_exhaustive]` so that a later SACM revision can add literals
/// without a breaking change.
///
/// # Examples
///
/// ```
/// use sacm::model::AssertionDeclaration;
///
/// let declaration: AssertionDeclaration = "needsSupport".parse()?;
/// assert_eq!(declaration, AssertionDeclaration::NeedsSupport);
/// assert_eq!(declaration.to_string(), "needsSupport");
/// assert_eq!(AssertionDeclaration::default(), AssertionDeclaration::Asserted);
///
/// // The Rust variant name is not a SACM spelling.
/// assert!("NeedsSupport".parse::<AssertionDeclaration>().is_err());
/// # Ok::<(), sacm::model::ParseAssertionDeclarationError>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssertionDeclaration {
    /// `asserted`: the default; the Assertion is asserted.
    #[default]
    Asserted,
    /// `needsSupport`: further argumentation has yet to be provided to support the Assertion.
    NeedsSupport,
    /// `assumed`: the author declares the Assertion assumed to be true, not supported by further argumentation.
    Assumed,
    /// `axiomatic`: the Assertion is axiomatically true, so no further argumentation is needed.
    Axiomatic,
    /// `defeated`: the Assertion is defeated by counter-evidence and/or argumentation.
    Defeated,
    /// `asCited`: the Assertion is cited, so its declaration should be derived, transitively,
    /// from the declaration of the cited Assertion. This crate does not derive it.
    AsCited,
}

impl AssertionDeclaration {
    /// Every literal, in the metamodel's order.
    ///
    /// ```
    /// use sacm::model::AssertionDeclaration;
    ///
    /// assert_eq!(AssertionDeclaration::ALL.len(), 6);
    /// assert_eq!(AssertionDeclaration::ALL[0], AssertionDeclaration::Asserted);
    /// ```
    pub const ALL: &[AssertionDeclaration] = &[
        Self::Asserted,
        Self::NeedsSupport,
        Self::Assumed,
        Self::Axiomatic,
        Self::Defeated,
        Self::AsCited,
    ];

    /// The literal's name, spelled as in the SACM 2.3 metamodel.
    ///
    /// ```
    /// use sacm::model::AssertionDeclaration;
    ///
    /// assert_eq!(AssertionDeclaration::AsCited.as_str(), "asCited");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asserted => "asserted",
            Self::NeedsSupport => "needsSupport",
            Self::Assumed => "assumed",
            Self::Axiomatic => "axiomatic",
            Self::Defeated => "defeated",
            Self::AsCited => "asCited",
        }
    }
}

impl fmt::Display for AssertionDeclaration {
    /// Writes [`AssertionDeclaration::as_str`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AssertionDeclaration {
    type Err = ParseAssertionDeclarationError;

    /// Parses a literal spelled exactly as [`AssertionDeclaration::as_str`]
    /// gives it.
    ///
    /// # Errors
    ///
    /// [`ParseAssertionDeclarationError`] if `s` is not one of the six
    /// spellings. Case and whitespace count.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|literal| literal.as_str() == s)
            .ok_or_else(|| ParseAssertionDeclarationError {
                input: s.to_owned(),
            })
    }
}

/// The error from parsing text that is not an [`AssertionDeclaration`]
/// literal.
///
/// [`input`](Self::input) returns the rejected text unchanged. The message
/// quotes it as Rust's `{:?}` does, so line breaks and other control
/// characters are escaped and the message stays on one line.
///
/// ```
/// use sacm::model::AssertionDeclaration;
///
/// let error = "Asserted".parse::<AssertionDeclaration>().unwrap_err();
/// assert_eq!(error.input(), "Asserted");
/// assert_eq!(error.to_string(), "invalid AssertionDeclaration literal \"Asserted\"");
/// ```
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseAssertionDeclarationError {
    input: String,
}

impl ParseAssertionDeclarationError {
    /// The text that failed to parse, exactly as given.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for ParseAssertionDeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `{:?}` quotes the input and escapes line breaks, so the message
        // stays on one line.
        write!(f, "invalid AssertionDeclaration literal {:?}", self.input)
    }
}

impl std::error::Error for ParseAssertionDeclarationError {}
