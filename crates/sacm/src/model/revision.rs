//! The revision of a document: one state of it within the process
//! (LIB-EDIT-007, ADR 0031).

use std::sync::atomic::{AtomicU64, Ordering};

/// One state of a [`Document`](super::Document) within the process: what
/// [`Document::revision`](super::Document::revision) returns.
///
/// - **Opaque.** It can be copied, compared for equality and hashed, and nothing else: no
///   order, no default, no public constructor.
/// - **For previews.** [`edit::apply_preview`](crate::edit::apply_preview) uses it to refuse
///   a [`Preview`](crate::edit::Preview) taken before the document changed (SACM-CMD-003).
/// - **New.** Every new, default or read document gets a revision no other document of the
///   process has. So does every successful edit, even one that changes nothing.
/// - **Kept.** `Clone` copies the revision, so a clone and its original share it until either
///   changes. A refused edit keeps it.
/// - **Not compared.** Document `==` ignores it (ADR 0020).
/// - **No number.** The values depend on how many documents the process has made, so compare
///   revisions only with each other.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let before = doc.revision();
/// let copy = doc.clone();
/// assert_eq!(copy.revision(), before);
///
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")))?;
/// assert_ne!(doc.revision(), before);
/// assert_eq!(copy.revision(), before);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Revision(u64);

/// The number of the next revision of the process. It starts at 1 and only
/// grows; 2^64 revisions are not reached.
static NEXT: AtomicU64 = AtomicU64::new(1);

impl Revision {
    /// A revision no document of the process has had.
    pub(crate) fn fresh() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// The revision written as `number` (ADR 0031: a JSON number).
    #[cfg(feature = "serde")]
    pub(crate) const fn from_number(number: u64) -> Self {
        Self(number)
    }

    /// The number the revision is written as.
    #[cfg(feature = "serde")]
    pub(crate) const fn number(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Revision;

    #[test]
    fn fresh_revisions_differ() {
        let (first, second) = (Revision::fresh(), Revision::fresh());
        assert_ne!(first, second);
    }
}
