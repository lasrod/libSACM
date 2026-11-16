//! The one list of what [`read`](super::read) refuses as
//! `ReadError::Unsupported` (LIB-API-011, ADR 0033).
//!
//! Content that does not conform is preserved, not refused
//! (LIB-COMPAT-003). What is left is conforming input this version does not
//! read yet, a limit of this version, input that only tolerant mode is to
//! read (a tolerance, which strict mode never applies, ADR 0022), and input
//! that does not conform and has nowhere to be kept. Each refusal names
//! which by one [`Unread`] entry, and its reason ends with that entry's
//! label, so a caller can tell a gap of the library from a defect of the
//! file. A tolerance's input need not be defective: a document in another
//! SACM namespace is not (D-NS-PIN).
//!
//! The xtask test `xtask/tests/interim_refusals.rs` keeps this list honest:
//! no other file of the crate builds the refusal, every `Interim` row is
//! open and gates SACM23-CP-001 and the compliance point of its own package
//! (every point, for a row of no package), and every `Tolerance` row is
//! open.

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
    /// Input strict mode never reads, which the row is to read in tolerant
    /// mode. It need not be defective, such as a document in another SACM
    /// namespace (D-NS-PIN).
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
                format!("(strict mode never reads it; {row} is to read it in tolerant mode)")
            }
            Self::Limit => "(a limit of this version)".to_owned(),
            Self::Defect => "(a defect in the input)".to_owned(),
        }
    }
}

/// What a refusal does not read: one entry per kind of input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unread {
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
    /// An `Artifact`.
    Artifact,
    /// A `Resource`.
    Resource,
    /// An `Activity`.
    Activity,
    /// An `Event`.
    Event,
    /// An `ArtifactAssetRelationship`.
    ArtifactAssetRelationship,
    /// A `Category`.
    Category,
    /// A `Term`.
    Term,
    /// An `Expression`.
    Expression,
    /// A `TerminologyGroup`.
    TerminologyGroup,
    /// XMI document metadata: an XMI-namespace element, `xmi:uuid`,
    /// `xmi:label` and `xmi:idref` on an object (on a role element, its
    /// reference form), `href` on a root or on a `name`, `description` or
    /// other text, the identity attributes of a reference element or of
    /// the `xmi:XMI` element, and `xsi:schemaLocation`.
    XmiMetadata,
    /// An `href` that is not `#id`, or `XLink` attributes on a reference
    /// element; or an unqualified `href` on a role element of a composite,
    /// typed or untyped, its reference form.
    CrossDocumentHref,
    /// A root outside the SACM 2.3 namespace.
    OtherSacmNamespace,
    /// A name or value typed `ExpressionLangString`.
    ExpressionLangString,
    /// An `XMI` element outside the XMI 2.5.1 namespace.
    OtherXmiNamespace,
    /// The §11.4 text spelling `argumentationElement`.
    TextSpelling,
    /// The machine-readable model's spelling `ArtifactAssertedRelationship`
    /// of the type `ArtifactAssetRelationship` (D-TEXT-MODEL).
    ModelSpelling,
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
    /// The entry of a kind this version does not read, if `kind` is one.
    pub(super) const fn kind(kind: ElementKind) -> Option<Self> {
        Some(match kind {
            ElementKind::ArtifactPackageInterface => Self::ArtifactPackageInterface,
            ElementKind::ArtifactPackageBinding => Self::ArtifactPackageBinding,
            ElementKind::TerminologyPackage => Self::TerminologyPackage,
            ElementKind::TerminologyPackageInterface => Self::TerminologyPackageInterface,
            ElementKind::TerminologyPackageBinding => Self::TerminologyPackageBinding,
            ElementKind::AssuranceCasePackageInterface => Self::AssuranceCasePackageInterface,
            ElementKind::AssuranceCasePackageBinding => Self::AssuranceCasePackageBinding,
            ElementKind::ArgumentPackageInterface => Self::ArgumentPackageInterface,
            ElementKind::ArgumentPackageBinding => Self::ArgumentPackageBinding,
            ElementKind::Artifact => Self::Artifact,
            ElementKind::Resource => Self::Resource,
            ElementKind::Activity => Self::Activity,
            ElementKind::Event => Self::Event,
            ElementKind::ArtifactAssetRelationship => Self::ArtifactAssetRelationship,
            ElementKind::Category => Self::Category,
            ElementKind::Term => Self::Term,
            ElementKind::Expression => Self::Expression,
            ElementKind::TerminologyGroup => Self::TerminologyGroup,
            _ => return None,
        })
    }

    /// Why it is refused, with the row that is to read it.
    pub(super) const fn refusal(self) -> Refusal {
        match self {
            Self::ArtifactPackageInterface => Refusal::Interim("SACM23-ART-012"),
            Self::ArtifactPackageBinding => Refusal::Interim("SACM23-ART-013"),
            Self::TerminologyPackage => Refusal::Interim("SACM23-TERM-001"),
            Self::TerminologyPackageInterface => Refusal::Interim("SACM23-TERM-007"),
            Self::TerminologyPackageBinding => Refusal::Interim("SACM23-TERM-008"),
            Self::AssuranceCasePackageInterface => Refusal::Interim("SACM23-AC-002"),
            Self::AssuranceCasePackageBinding => Refusal::Interim("SACM23-AC-003"),
            Self::ArgumentPackageInterface => Refusal::Interim("SACM23-ARG-014"),
            Self::ArgumentPackageBinding => Refusal::Interim("SACM23-ARG-015"),
            Self::Artifact => Refusal::Interim("SACM23-ART-004"),
            Self::Resource => Refusal::Interim("SACM23-ART-005"),
            Self::Activity => Refusal::Interim("SACM23-ART-006"),
            Self::Event => Refusal::Interim("SACM23-ART-007"),
            Self::ArtifactAssetRelationship => Refusal::Interim("SACM23-ART-010"),
            Self::Category => Refusal::Interim("SACM23-TERM-002"),
            Self::Term => Refusal::Interim("SACM23-TERM-004"),
            Self::Expression => Refusal::Interim("SACM23-TERM-005"),
            Self::TerminologyGroup => Refusal::Interim("SACM23-TERM-006"),
            Self::XmiMetadata => Refusal::Interim("SACM23-XMI-019"),
            Self::CrossDocumentHref => Refusal::Interim("SACM23-XMI-027"),
            Self::OtherSacmNamespace => Refusal::Tolerance("LIB-COMPAT-004"),
            Self::ExpressionLangString => Refusal::Interim("SACM23-BASE-009"),
            Self::OtherXmiNamespace => Refusal::Tolerance("LIB-COMPAT-002"),
            Self::TextSpelling | Self::ModelSpelling => Refusal::Tolerance("SACM23-XMI-009"),
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
            "x (strict mode never reads it; SACM23-XMI-009 is to read it in tolerant mode)"
        );
        assert_eq!(reason(Unread::Capacity), "x (a limit of this version)");
        assert_eq!(reason(Unread::WrapperContent), "x (a defect in the input)");
    }

    #[test]
    fn exactly_the_eighteen_unread_kinds_have_an_entry() {
        let unread: Vec<ElementKind> = ElementKind::ALL
            .iter()
            .copied()
            .filter(|&kind| Unread::kind(kind).is_some())
            .collect();
        assert_eq!(unread.len(), 18);
        let packages = unread.iter().filter(|kind| kind.is_package()).count();
        assert_eq!((packages, unread.len() - packages), (9, 9));
        assert_eq!(Unread::kind(ElementKind::ArtifactPackage), None);
        assert_eq!(
            Unread::kind(ElementKind::Artifact).map(Unread::refusal),
            Some(Refusal::Interim("SACM23-ART-004"))
        );
        assert_eq!(
            Unread::ModelSpelling.refusal(),
            Refusal::Tolerance("SACM23-XMI-009")
        );
        for kind in unread {
            let entry = Unread::kind(kind).map(Unread::refusal);
            assert!(matches!(entry, Some(Refusal::Interim(_))), "{kind}");
        }
    }
}
