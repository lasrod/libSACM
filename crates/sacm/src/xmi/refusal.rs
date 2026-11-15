//! The one list of what [`read`](super::read) refuses as
//! `ReadError::Unsupported` (LIB-API-011, ADR 0033).
//!
//! Content that does not conform is preserved, not refused
//! (LIB-COMPAT-003). What is left is conforming input this version does not
//! read yet, a limit of this version, and input that does not conform and
//! has nowhere to be kept. Each refusal names which by one [`Unread`]
//! entry, and its reason ends with that entry's label, so a caller can tell
//! a gap of the library from a defect of the file.
//!
//! The xtask test `xtask/tests/interim_refusals.rs` keeps this list honest:
//! no other file of the crate builds the refusal, every `Interim` row is
//! open and gates every compliance point, and every `Tolerance` row is open.

use super::ReadError;
use super::input::locate;
use crate::diagnostic::SourceLocation;
use crate::model::ElementKind;

/// Why input is refused rather than read or preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// Input this version does not read yet, and the row reads: conforming
    /// input, and XMI-namespace elements that XMI.xsd does not define
    /// (SACM23-XMI-019).
    Interim(&'static str),
    /// Input that does not conform, which the row is to read in tolerant
    /// mode.
    Tolerance(&'static str),
    /// A limit of this version.
    Limit,
    /// Input that does not conform and has nowhere to be kept.
    Defect,
}

impl Refusal {
    /// The label a reason ends with.
    fn label(self) -> String {
        match self {
            Self::Interim(row) => format!("(a limit of this version; {row} reads it)"),
            Self::Tolerance(row) => {
                format!("(a defect in the input; {row} is to read it in tolerant mode)")
            }
            Self::Limit => "(a limit of this version)".to_owned(),
            Self::Defect => "(a defect in the input)".to_owned(),
        }
    }
}

/// What a refusal does not read: one entry per kind of input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unread {
    /// An `ArtifactPackage`.
    ArtifactPackage,
    /// An `ArtifactPackageInterface`.
    ArtifactPackageInterface,
    /// An `ArtifactPackageBinding`.
    ArtifactPackageBinding,
    /// A `TerminologyPackage`.
    TerminologyPackage,
    /// A `TerminologyPackageInterface`.
    TerminologyPackageInterface,
    /// A `TerminologyPackageBinding`.
    TerminologyPackageBinding,
    /// An `AssuranceCasePackageInterface`.
    AssuranceCasePackageInterface,
    /// An `AssuranceCasePackageBinding`.
    AssuranceCasePackageBinding,
    /// An `ArgumentPackageInterface`.
    ArgumentPackageInterface,
    /// An `ArgumentPackageBinding`.
    ArgumentPackageBinding,
    /// XMI document metadata: an XMI-namespace element, `xmi:uuid`,
    /// `xmi:label`, `href` and `xmi:idref` on an object, the identity
    /// attributes of a reference element or of the `xmi:XMI` element, and
    /// `xsi:schemaLocation`.
    XmiMetadata,
    /// An `href` that is not `#id`, or `XLink` attributes on a reference
    /// element; or an `href` on an untyped role element of a composite,
    /// its reference form.
    CrossDocumentHref,
    /// A root outside the SACM 2.3 namespace.
    OtherSacmNamespace,
    /// A name or value typed `ExpressionLangString`.
    ExpressionLangString,
    /// An `XMI` element outside the XMI 2.5.1 namespace.
    OtherXmiNamespace,
    /// The §11.4 text spelling `argumentationElement`.
    TextSpelling,
    /// An encoding other than UTF-8, ASCII and UTF-16 with a mark.
    Encoding,
    /// More nodes, attributes or namespaces than the parser can hold.
    Capacity,
    /// An element the document could not hold, which checked input never
    /// gives.
    Unheld,
    /// An attribute or text on the `xmi:XMI` element, which has no element
    /// to keep it.
    WrapperContent,
    /// A preserved attribute whose prefix `sacm` or `xmi` is bound to
    /// another namespace, which the written start tag binds to SACM 2.3 or
    /// XMI 2.5.1.
    UnwritablePrefix,
}

impl Unread {
    /// The entry of a package kind this version does not read, if `kind`
    /// is one.
    pub(super) const fn kind(kind: ElementKind) -> Option<Self> {
        Some(match kind {
            ElementKind::ArtifactPackage => Self::ArtifactPackage,
            ElementKind::ArtifactPackageInterface => Self::ArtifactPackageInterface,
            ElementKind::ArtifactPackageBinding => Self::ArtifactPackageBinding,
            ElementKind::TerminologyPackage => Self::TerminologyPackage,
            ElementKind::TerminologyPackageInterface => Self::TerminologyPackageInterface,
            ElementKind::TerminologyPackageBinding => Self::TerminologyPackageBinding,
            ElementKind::AssuranceCasePackageInterface => Self::AssuranceCasePackageInterface,
            ElementKind::AssuranceCasePackageBinding => Self::AssuranceCasePackageBinding,
            ElementKind::ArgumentPackageInterface => Self::ArgumentPackageInterface,
            ElementKind::ArgumentPackageBinding => Self::ArgumentPackageBinding,
            _ => return None,
        })
    }

    /// Why it is refused, with the row that is to read it.
    pub(super) const fn refusal(self) -> Refusal {
        match self {
            Self::ArtifactPackage => Refusal::Interim("SACM23-ART-001"),
            Self::ArtifactPackageInterface => Refusal::Interim("SACM23-ART-012"),
            Self::ArtifactPackageBinding => Refusal::Interim("SACM23-ART-013"),
            Self::TerminologyPackage => Refusal::Interim("SACM23-TERM-001"),
            Self::TerminologyPackageInterface => Refusal::Interim("SACM23-TERM-007"),
            Self::TerminologyPackageBinding => Refusal::Interim("SACM23-TERM-008"),
            Self::AssuranceCasePackageInterface => Refusal::Interim("SACM23-AC-002"),
            Self::AssuranceCasePackageBinding => Refusal::Interim("SACM23-AC-003"),
            Self::ArgumentPackageInterface => Refusal::Interim("SACM23-ARG-014"),
            Self::ArgumentPackageBinding => Refusal::Interim("SACM23-ARG-015"),
            Self::XmiMetadata => Refusal::Interim("SACM23-XMI-019"),
            Self::CrossDocumentHref => Refusal::Interim("SACM23-XMI-027"),
            Self::OtherSacmNamespace => Refusal::Tolerance("LIB-COMPAT-004"),
            Self::ExpressionLangString => Refusal::Interim("SACM23-BASE-009"),
            Self::OtherXmiNamespace => Refusal::Tolerance("LIB-COMPAT-002"),
            Self::TextSpelling => Refusal::Tolerance("SACM23-XMI-009"),
            Self::Encoding | Self::Capacity | Self::Unheld => Refusal::Limit,
            Self::WrapperContent | Self::UnwritablePrefix => Refusal::Defect,
        }
    }
}

/// The refusal of `unread` at `location`: `reason`, which says what is not
/// read, followed by the entry's label. The one place the crate builds
/// `ReadError::Unsupported`.
pub(super) fn unsupported_at(
    location: Option<SourceLocation>,
    unread: Unread,
    reason: &str,
) -> ReadError {
    ReadError::Unsupported {
        location,
        reason: format!("{reason} {}", unread.refusal().label()),
    }
}

/// The refusal of `unread` at byte `offset` of `text`, as
/// [`unsupported_at`] builds it.
pub(super) fn unsupported(text: &str, offset: usize, unread: Unread, reason: &str) -> ReadError {
    unsupported_at(locate(text, offset), unread, reason)
}

#[cfg(test)]
mod tests {
    use super::{Refusal, Unread, unsupported_at};
    use crate::model::ElementKind;
    use crate::xmi::ReadError;

    #[test]
    fn every_reason_ends_with_its_label() {
        let reason = |unread| match unsupported_at(None, unread, "x") {
            ReadError::Unsupported { reason, .. } => reason,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            reason(Unread::XmiMetadata),
            "x (a limit of this version; SACM23-XMI-019 reads it)"
        );
        assert_eq!(
            reason(Unread::TextSpelling),
            "x (a defect in the input; SACM23-XMI-009 is to read it in tolerant mode)"
        );
        assert_eq!(reason(Unread::Capacity), "x (a limit of this version)");
        assert_eq!(reason(Unread::WrapperContent), "x (a defect in the input)");
    }

    #[test]
    fn exactly_the_ten_unread_package_kinds_have_an_entry() {
        let unread: Vec<ElementKind> = ElementKind::ALL
            .iter()
            .copied()
            .filter(|&kind| Unread::kind(kind).is_some())
            .collect();
        assert_eq!(unread.len(), 10);
        assert!(unread.iter().all(|kind| kind.is_package()));
        for kind in unread {
            let entry = Unread::kind(kind).map(Unread::refusal);
            assert!(matches!(entry, Some(Refusal::Interim(_))), "{kind}");
        }
    }
}
