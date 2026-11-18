//! The serialized form of [`EditError`] (LIB-API-006; ADR 0030 R1, R5 and
//! R7).
//!
//! An `EditError` names its end, feature and expected class with
//! `&'static str`s. It serializes through [`EditErrorRecord`], which writes
//! the variant's catalog `code` right after the `"type"` tag, and it reads
//! back through it: each name maps to the static name the crate uses, and
//! any other name, or a code that is not the variant's, is refused.

use serde::{Deserialize, Serialize};

use super::EditError;
use super::set::{ASSERTION_DECLARATION, GID, NAME};
use crate::diagnostic::Code;
use crate::model::{
    AbstractClass, ElementId, ElementKind, Flag, ReferenceEnd, Revision, StringAttribute, TextRole,
};

/// A name as serialized: an end, feature or class name, which reads back to
/// the crate's static name.
type Name = String;

/// An [`EditError`] as serialized: the same variants and fields, each with
/// the variant's `code` first.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub(super) enum EditErrorRecord {
    /// [`EditError::ParentNotFound`].
    ParentNotFound { code: Code, parent: ElementId },
    /// [`EditError::InvalidParent`].
    InvalidParent {
        code: Code,
        parent: ElementId,
        parent_kind: ElementKind,
        kind: ElementKind,
    },
    /// [`EditError::InvalidId`].
    InvalidId { code: Code, id: ElementId },
    /// [`EditError::DuplicateId`].
    DuplicateId { code: Code, id: ElementId },
    /// [`EditError::TooFewReferences`].
    TooFewReferences {
        code: Code,
        end: Name,
        given: usize,
        required: usize,
    },
    /// [`EditError::TooManyReferences`].
    TooManyReferences {
        code: Code,
        end: Name,
        given: usize,
        allowed: usize,
    },
    /// [`EditError::InvalidReference`].
    InvalidReference {
        code: Code,
        end: Name,
        id: ElementId,
    },
    /// [`EditError::ReferenceNotFound`].
    ReferenceNotFound {
        code: Code,
        end: Name,
        id: ElementId,
    },
    /// [`EditError::WrongReferenceType`].
    WrongReferenceType {
        code: Code,
        end: Name,
        id: ElementId,
        kind: ElementKind,
        expected: Name,
    },
    /// [`EditError::ElementNotFound`].
    ElementNotFound { code: Code, element: ElementId },
    /// [`EditError::FeatureNotApplicable`].
    FeatureNotApplicable {
        code: Code,
        element: ElementId,
        kind: ElementKind,
        feature: Name,
    },
    /// [`EditError::DerivedEnd`].
    DerivedEnd {
        code: Code,
        element: ElementId,
        kind: ElementKind,
        end: Name,
    },
    /// [`EditError::ReferenceNotHeld`].
    ReferenceNotHeld {
        code: Code,
        end: Name,
        id: ElementId,
    },
    /// [`EditError::ChildNotFound`]; `parent` is required, `null` for the
    /// roots.
    ChildNotFound {
        code: Code,
        #[serde(deserialize_with = "serde::Deserialize::deserialize")]
        parent: Option<ElementId>,
        child: ElementId,
    },
    /// [`EditError::InvalidOrder`]; `repeated` is required, `null` for an
    /// empty list.
    InvalidOrder {
        code: Code,
        #[serde(deserialize_with = "serde::Deserialize::deserialize")]
        repeated: Option<ElementId>,
    },
    /// [`EditError::Referenced`].
    Referenced {
        code: Code,
        holder: ElementId,
        end: Name,
        id: ElementId,
    },
    /// [`EditError::PreviewExpired`]; each revision is a JSON number (ADR
    /// 0031).
    PreviewExpired {
        code: Code,
        previewed: u64,
        current: u64,
    },
    /// [`EditError::NotEmpty`].
    NotEmpty {
        code: Code,
        element: ElementId,
        contained: usize,
    },
    /// [`EditError::CrossPackageReference`].
    CrossPackageReference {
        code: Code,
        holder: ElementId,
        end: Name,
        id: ElementId,
    },
    /// [`EditError::InvalidRoot`].
    InvalidRoot {
        code: Code,
        element: ElementId,
        kind: ElementKind,
    },
    /// [`EditError::ParentInSubtree`].
    ParentInSubtree {
        code: Code,
        element: ElementId,
        parent: ElementId,
    },
    /// [`EditError::PositionOutOfRange`]; `parent` is required, `null` for
    /// the roots.
    PositionOutOfRange {
        code: Code,
        #[serde(deserialize_with = "serde::Deserialize::deserialize")]
        parent: Option<ElementId>,
        position: usize,
        children: usize,
    },
}

impl From<EditError> for EditErrorRecord {
    #[expect(clippy::too_many_lines, reason = "one arm per EditError variant")]
    fn from(error: EditError) -> Self {
        let code = error.code();
        match error {
            EditError::ParentNotFound { parent } => Self::ParentNotFound { code, parent },
            EditError::InvalidParent {
                parent,
                parent_kind,
                kind,
            } => Self::InvalidParent {
                code,
                parent,
                parent_kind,
                kind,
            },
            EditError::InvalidId { id } => Self::InvalidId { code, id },
            EditError::DuplicateId { id } => Self::DuplicateId { code, id },
            EditError::TooFewReferences {
                end,
                given,
                required,
            } => Self::TooFewReferences {
                code,
                end: end.to_owned(),
                given,
                required,
            },
            EditError::TooManyReferences {
                end,
                given,
                allowed,
            } => Self::TooManyReferences {
                code,
                end: end.to_owned(),
                given,
                allowed,
            },
            EditError::InvalidReference { end, id } => Self::InvalidReference {
                code,
                end: end.to_owned(),
                id,
            },
            EditError::ReferenceNotFound { end, id } => Self::ReferenceNotFound {
                code,
                end: end.to_owned(),
                id,
            },
            EditError::WrongReferenceType {
                end,
                id,
                kind,
                expected,
            } => Self::WrongReferenceType {
                code,
                end: end.to_owned(),
                id,
                kind,
                expected: expected.to_owned(),
            },
            EditError::ElementNotFound { element } => Self::ElementNotFound { code, element },
            EditError::FeatureNotApplicable {
                element,
                kind,
                feature,
            } => Self::FeatureNotApplicable {
                code,
                element,
                kind,
                feature: feature.to_owned(),
            },
            EditError::DerivedEnd { element, kind, end } => Self::DerivedEnd {
                code,
                element,
                kind,
                end: end.to_owned(),
            },
            EditError::ReferenceNotHeld { end, id } => Self::ReferenceNotHeld {
                code,
                end: end.to_owned(),
                id,
            },
            EditError::ChildNotFound { parent, child } => Self::ChildNotFound {
                code,
                parent,
                child,
            },
            EditError::InvalidOrder { repeated } => Self::InvalidOrder { code, repeated },
            EditError::Referenced { holder, end, id } => Self::Referenced {
                code,
                holder,
                end: end.to_owned(),
                id,
            },
            EditError::PreviewExpired { previewed, current } => Self::PreviewExpired {
                code,
                previewed: previewed.number(),
                current: current.number(),
            },
            EditError::NotEmpty { element, contained } => Self::NotEmpty {
                code,
                element,
                contained,
            },
            EditError::CrossPackageReference { holder, end, id } => Self::CrossPackageReference {
                code,
                holder,
                end: end.to_owned(),
                id,
            },
            EditError::InvalidRoot { element, kind } => Self::InvalidRoot {
                code,
                element,
                kind,
            },
            EditError::ParentInSubtree { element, parent } => Self::ParentInSubtree {
                code,
                element,
                parent,
            },
            EditError::PositionOutOfRange {
                parent,
                position,
                children,
            } => Self::PositionOutOfRange {
                code,
                parent,
                position,
                children,
            },
        }
    }
}

impl TryFrom<EditErrorRecord> for EditError {
    type Error = String;

    /// The refusal the record describes, once its names are known and its
    /// code is its variant's.
    #[expect(clippy::too_many_lines, reason = "one arm per EditError variant")]
    fn try_from(record: EditErrorRecord) -> Result<Self, String> {
        type R = EditErrorRecord;
        let (code, error) = match record {
            R::ParentNotFound { code, parent } => (code, Self::ParentNotFound { parent }),
            R::InvalidParent {
                code,
                parent,
                parent_kind,
                kind,
            } => (
                code,
                Self::InvalidParent {
                    parent,
                    parent_kind,
                    kind,
                },
            ),
            R::InvalidId { code, id } => (code, Self::InvalidId { id }),
            R::DuplicateId { code, id } => (code, Self::DuplicateId { id }),
            R::TooFewReferences {
                code,
                end,
                given,
                required,
            } => (
                code,
                Self::TooFewReferences {
                    end: end_name(&end)?,
                    given,
                    required,
                },
            ),
            R::TooManyReferences {
                code,
                end,
                given,
                allowed,
            } => (
                code,
                Self::TooManyReferences {
                    end: end_name(&end)?,
                    given,
                    allowed,
                },
            ),
            R::InvalidReference { code, end, id } => (
                code,
                Self::InvalidReference {
                    end: end_name(&end)?,
                    id,
                },
            ),
            R::ReferenceNotFound { code, end, id } => (
                code,
                Self::ReferenceNotFound {
                    end: end_name(&end)?,
                    id,
                },
            ),
            R::WrongReferenceType {
                code,
                end,
                id,
                kind,
                expected,
            } => (
                code,
                Self::WrongReferenceType {
                    end: end_name(&end)?,
                    id,
                    kind,
                    expected: class_name(&expected)?,
                },
            ),
            R::ElementNotFound { code, element } => (code, Self::ElementNotFound { element }),
            R::FeatureNotApplicable {
                code,
                element,
                kind,
                feature,
            } => (
                code,
                Self::FeatureNotApplicable {
                    element,
                    kind,
                    feature: feature_name(&feature)?,
                },
            ),
            R::DerivedEnd {
                code,
                element,
                kind,
                end,
            } => (
                code,
                Self::DerivedEnd {
                    element,
                    kind,
                    end: end_name(&end)?,
                },
            ),
            R::ReferenceNotHeld { code, end, id } => (
                code,
                Self::ReferenceNotHeld {
                    end: end_name(&end)?,
                    id,
                },
            ),
            R::ChildNotFound {
                code,
                parent,
                child,
            } => (code, Self::ChildNotFound { parent, child }),
            R::InvalidOrder { code, repeated } => (code, Self::InvalidOrder { repeated }),
            R::Referenced {
                code,
                holder,
                end,
                id,
            } => (
                code,
                Self::Referenced {
                    holder,
                    end: end_name(&end)?,
                    id,
                },
            ),
            R::PreviewExpired {
                code,
                previewed,
                current,
            } => (
                code,
                Self::PreviewExpired {
                    previewed: Revision::from_number(previewed),
                    current: Revision::from_number(current),
                },
            ),
            R::NotEmpty {
                code,
                element,
                contained,
            } => (code, Self::NotEmpty { element, contained }),
            R::CrossPackageReference {
                code,
                holder,
                end,
                id,
            } => (
                code,
                Self::CrossPackageReference {
                    holder,
                    end: end_name(&end)?,
                    id,
                },
            ),
            R::InvalidRoot {
                code,
                element,
                kind,
            } => (code, Self::InvalidRoot { element, kind }),
            R::ParentInSubtree {
                code,
                element,
                parent,
            } => (code, Self::ParentInSubtree { element, parent }),
            R::PositionOutOfRange {
                code,
                parent,
                position,
                children,
            } => (
                code,
                Self::PositionOutOfRange {
                    parent,
                    position,
                    children,
                },
            ),
        };
        if code == error.code() {
            Ok(error)
        } else {
            Err(format!(
                "the code {code} is not the code of the refusal, {}",
                error.code()
            ))
        }
    }
}

/// Serializes as ADR 0030 shapes it: `"type"`, the variant's name, then
/// `"code"`, its catalog code, then the variant's fields (R1, R7).
impl Serialize for EditError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        EditErrorRecord::from(self.clone()).serialize(serializer)
    }
}

/// Deserializes the form it serializes to. An unknown `"type"`, key, end,
/// feature or class name, and a `"code"` that is not the variant's, are
/// refused (ADR 0030 R4, R5, R7).
//
// Written by hand: a derive would borrow the `&'static str` fields, and so
// read only `'static` input.
impl<'de> Deserialize<'de> for EditError {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let record = EditErrorRecord::deserialize(deserializer)?;
        Self::try_from(record).map_err(serde::de::Error::custom)
    }
}

/// The static name among `names` that `text` spells exactly.
fn known(
    what: &str,
    text: &str,
    mut names: impl Iterator<Item = &'static str>,
) -> Result<&'static str, String> {
    names
        .find(|name| *name == text)
        .ok_or_else(|| format!("unknown {what} {text:?}"))
}

/// The XMI name of the reference end `text` names.
fn end_name(text: &str) -> Result<&'static str, String> {
    known(
        "reference end",
        text,
        ReferenceEnd::ALL.iter().copied().map(ReferenceEnd::name),
    )
}

/// The XMI name of the feature `text` names: one an operation edits (ADR
/// 0029), a flag or a reference end.
fn feature_name(text: &str) -> Result<&'static str, String> {
    let features = [NAME, GID, ASSERTION_DECLARATION];
    known(
        "feature",
        text,
        features
            .into_iter()
            .chain(TextRole::ALL.into_iter().map(TextRole::name))
            .chain(StringAttribute::ALL.into_iter().map(StringAttribute::name))
            .chain(Flag::ALL.iter().copied().map(Flag::name))
            .chain(ReferenceEnd::ALL.iter().copied().map(ReferenceEnd::name)),
    )
}

/// The SACM class name `text` names: an abstract class or a kind, as an
/// end's declared type is.
fn class_name(text: &str) -> Result<&'static str, String> {
    known(
        "class",
        text,
        AbstractClass::ALL
            .iter()
            .map(|class| class.name())
            .chain(ElementKind::ALL.iter().map(|kind| kind.name())),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::edit::{EditError, Operation};

    /// The golden operations of the serde tests.
    const OPERATIONS: &str = include_str!("../../tests/data/golden/serde/operations.json");
    /// The golden edit errors of the serde tests.
    const EDIT_ERRORS: &str = include_str!("../../tests/data/golden/serde/edit-errors.json");

    /// Whether `haystack` holds `needle`, at compile time.
    const fn contains(haystack: &str, needle: &str) -> bool {
        let (haystack, needle) = (haystack.as_bytes(), needle.as_bytes());
        let mut start = 0;
        while start + needle.len() <= haystack.len() {
            let mut matched = 0;
            while matched < needle.len() && haystack[start + matched] == needle[matched] {
                matched += 1;
            }
            if matched == needle.len() {
                return true;
            }
            start += 1;
        }
        false
    }

    /// `$name`, which fails to compile unless the golden file `$golden`
    /// holds a sample tagged `"type": "$name"`.
    macro_rules! sampled {
        ($golden:expr, $name:literal) => {{
            const _: () = assert!(
                contains($golden, concat!("\"type\": \"", $name, "\"")),
                concat!("the golden file has no sample of ", $name)
            );
            $name
        }};
    }

    /// The variant name of `operation`. The match has no wildcard, so a new
    /// variant does not compile until it has an arm, and its arm does not
    /// compile until `operations.json` has a sample of it.
    fn sampled_operation(operation: &Operation) -> &'static str {
        match operation {
            Operation::CreateAssuranceCasePackage(_) => {
                sampled!(OPERATIONS, "CreateAssuranceCasePackage")
            }
            Operation::CreateAssuranceCasePackageInterface(_) => {
                sampled!(OPERATIONS, "CreateAssuranceCasePackageInterface")
            }
            Operation::CreateAssuranceCasePackageBinding(_) => {
                sampled!(OPERATIONS, "CreateAssuranceCasePackageBinding")
            }
            Operation::CreateArgumentPackage(_) => sampled!(OPERATIONS, "CreateArgumentPackage"),
            Operation::CreateArgumentPackageInterface(_) => {
                sampled!(OPERATIONS, "CreateArgumentPackageInterface")
            }
            Operation::CreateArgumentPackageBinding(_) => {
                sampled!(OPERATIONS, "CreateArgumentPackageBinding")
            }
            Operation::CreateClaim(_) => sampled!(OPERATIONS, "CreateClaim"),
            Operation::CreateArgumentReasoning(_) => {
                sampled!(OPERATIONS, "CreateArgumentReasoning")
            }
            Operation::CreateAssertedInference(_) => {
                sampled!(OPERATIONS, "CreateAssertedInference")
            }
            Operation::CreateAssertedContext(_) => sampled!(OPERATIONS, "CreateAssertedContext"),
            Operation::CreateArtifactReference(_) => {
                sampled!(OPERATIONS, "CreateArtifactReference")
            }
            Operation::CreateArgumentGroup(_) => sampled!(OPERATIONS, "CreateArgumentGroup"),
            Operation::CreateAssertedEvidence(_) => sampled!(OPERATIONS, "CreateAssertedEvidence"),
            Operation::CreateAssertedArtifactSupport(_) => {
                sampled!(OPERATIONS, "CreateAssertedArtifactSupport")
            }
            Operation::CreateAssertedArtifactContext(_) => {
                sampled!(OPERATIONS, "CreateAssertedArtifactContext")
            }
            Operation::CreateDescription(_) => sampled!(OPERATIONS, "CreateDescription"),
            Operation::CreateImplementationConstraint(_) => {
                sampled!(OPERATIONS, "CreateImplementationConstraint")
            }
            Operation::CreateNote(_) => sampled!(OPERATIONS, "CreateNote"),
            Operation::CreateTaggedValue(_) => sampled!(OPERATIONS, "CreateTaggedValue"),
            Operation::CreateArtifactPackage(_) => sampled!(OPERATIONS, "CreateArtifactPackage"),
            Operation::CreateArtifactPackageInterface(_) => {
                sampled!(OPERATIONS, "CreateArtifactPackageInterface")
            }
            Operation::CreateArtifactPackageBinding(_) => {
                sampled!(OPERATIONS, "CreateArtifactPackageBinding")
            }
            Operation::CreateProperty(_) => sampled!(OPERATIONS, "CreateProperty"),
            Operation::CreateTechnique(_) => sampled!(OPERATIONS, "CreateTechnique"),
            Operation::CreateParticipant(_) => sampled!(OPERATIONS, "CreateParticipant"),
            Operation::CreateArtifactGroup(_) => sampled!(OPERATIONS, "CreateArtifactGroup"),
            Operation::CreateArtifact(_) => sampled!(OPERATIONS, "CreateArtifact"),
            Operation::CreateEvent(_) => sampled!(OPERATIONS, "CreateEvent"),
            Operation::CreateResource(_) => sampled!(OPERATIONS, "CreateResource"),
            Operation::CreateActivity(_) => sampled!(OPERATIONS, "CreateActivity"),
            Operation::CreateArtifactAssetRelationship(_) => {
                sampled!(OPERATIONS, "CreateArtifactAssetRelationship")
            }
            Operation::CreateTerminologyPackage(_) => {
                sampled!(OPERATIONS, "CreateTerminologyPackage")
            }
            Operation::CreateTerminologyPackageInterface(_) => {
                sampled!(OPERATIONS, "CreateTerminologyPackageInterface")
            }
            Operation::CreateTerminologyPackageBinding(_) => {
                sampled!(OPERATIONS, "CreateTerminologyPackageBinding")
            }
            Operation::CreateCategory(_) => sampled!(OPERATIONS, "CreateCategory"),
            Operation::CreateTerm(_) => sampled!(OPERATIONS, "CreateTerm"),
            Operation::CreateExpression(_) => sampled!(OPERATIONS, "CreateExpression"),
            Operation::CreateTerminologyGroup(_) => {
                sampled!(OPERATIONS, "CreateTerminologyGroup")
            }
            Operation::SetName(_) => sampled!(OPERATIONS, "SetName"),
            Operation::SetGid(_) => sampled!(OPERATIONS, "SetGid"),
            Operation::SetFlag(_) => sampled!(OPERATIONS, "SetFlag"),
            Operation::SetAssertionDeclaration(_) => {
                sampled!(OPERATIONS, "SetAssertionDeclaration")
            }
            Operation::SetContent(_) => sampled!(OPERATIONS, "SetContent"),
            Operation::SetKey(_) => sampled!(OPERATIONS, "SetKey"),
            Operation::SetVersion(_) => sampled!(OPERATIONS, "SetVersion"),
            Operation::SetDate(_) => sampled!(OPERATIONS, "SetDate"),
            Operation::SetLocation(_) => sampled!(OPERATIONS, "SetLocation"),
            Operation::SetStartTime(_) => sampled!(OPERATIONS, "SetStartTime"),
            Operation::SetEndTime(_) => sampled!(OPERATIONS, "SetEndTime"),
            Operation::SetValue(_) => sampled!(OPERATIONS, "SetValue"),
            Operation::SetExternalReference(_) => sampled!(OPERATIONS, "SetExternalReference"),
            Operation::SetReferences(_) => sampled!(OPERATIONS, "SetReferences"),
            Operation::AddReference(_) => sampled!(OPERATIONS, "AddReference"),
            Operation::RemoveReference(_) => sampled!(OPERATIONS, "RemoveReference"),
            Operation::ReorderElements(_) => sampled!(OPERATIONS, "ReorderElements"),
            Operation::MoveElement(_) => sampled!(OPERATIONS, "MoveElement"),
            // Not serialized, so it has no sample (ADR 0030 amendment 2).
            Operation::CopyElements(_) => "CopyElements",
            Operation::DeleteElement(_) => sampled!(OPERATIONS, "DeleteElement"),
        }
    }

    /// The variant name of `error`. The match has no wildcard, so a new
    /// variant does not compile until it has an arm, and its arm does not
    /// compile until `edit-errors.json` has a sample of it.
    fn sampled_error(error: &EditError) -> &'static str {
        match error {
            EditError::ParentNotFound { .. } => sampled!(EDIT_ERRORS, "ParentNotFound"),
            EditError::InvalidParent { .. } => sampled!(EDIT_ERRORS, "InvalidParent"),
            EditError::InvalidId { .. } => sampled!(EDIT_ERRORS, "InvalidId"),
            EditError::DuplicateId { .. } => sampled!(EDIT_ERRORS, "DuplicateId"),
            EditError::TooFewReferences { .. } => sampled!(EDIT_ERRORS, "TooFewReferences"),
            EditError::TooManyReferences { .. } => sampled!(EDIT_ERRORS, "TooManyReferences"),
            EditError::InvalidReference { .. } => sampled!(EDIT_ERRORS, "InvalidReference"),
            EditError::ReferenceNotFound { .. } => sampled!(EDIT_ERRORS, "ReferenceNotFound"),
            EditError::WrongReferenceType { .. } => sampled!(EDIT_ERRORS, "WrongReferenceType"),
            EditError::ElementNotFound { .. } => sampled!(EDIT_ERRORS, "ElementNotFound"),
            EditError::FeatureNotApplicable { .. } => {
                sampled!(EDIT_ERRORS, "FeatureNotApplicable")
            }
            EditError::DerivedEnd { .. } => sampled!(EDIT_ERRORS, "DerivedEnd"),
            EditError::ReferenceNotHeld { .. } => sampled!(EDIT_ERRORS, "ReferenceNotHeld"),
            EditError::ChildNotFound { .. } => sampled!(EDIT_ERRORS, "ChildNotFound"),
            EditError::InvalidOrder { .. } => sampled!(EDIT_ERRORS, "InvalidOrder"),
            EditError::Referenced { .. } => sampled!(EDIT_ERRORS, "Referenced"),
            EditError::PreviewExpired { .. } => sampled!(EDIT_ERRORS, "PreviewExpired"),
            EditError::NotEmpty { .. } => sampled!(EDIT_ERRORS, "NotEmpty"),
            EditError::CrossPackageReference { .. } => {
                sampled!(EDIT_ERRORS, "CrossPackageReference")
            }
            EditError::InvalidRoot { .. } => sampled!(EDIT_ERRORS, "InvalidRoot"),
            EditError::ParentInSubtree { .. } => sampled!(EDIT_ERRORS, "ParentInSubtree"),
            EditError::PositionOutOfRange { .. } => sampled!(EDIT_ERRORS, "PositionOutOfRange"),
        }
    }

    #[test]
    fn every_operation_and_edit_error_variant_has_a_golden_sample_under_its_name() {
        let operations: Vec<Operation> =
            serde_json::from_str(OPERATIONS).expect("operations.json reads back");
        let mut names = BTreeSet::new();
        for operation in &operations {
            assert_eq!(sampled_operation(operation), operation.operation_name());
            names.insert(sampled_operation(operation));
        }
        assert!(!names.is_empty());

        let errors: Vec<EditError> =
            serde_json::from_str(EDIT_ERRORS).expect("edit-errors.json reads back");
        let mut variants = BTreeSet::new();
        for error in &errors {
            let name = sampled_error(error);
            let written = serde_json::to_string(error).expect("an EditError serializes");
            assert!(
                written.starts_with(&format!("{{\"type\":\"{name}\",")),
                "{written} is not tagged {name}"
            );
            assert!(
                format!("{error:?}").starts_with(name),
                "{error:?} is not {name}"
            );
            variants.insert(name);
        }
        assert!(!variants.is_empty());
    }

    #[test]
    fn contains_finds_only_what_is_there() {
        assert!(contains("abc", "abc"));
        assert!(contains("xabcx", "bc"));
        assert!(contains("abc", ""));
        assert!(!contains("abc", "abd"));
        assert!(!contains("ab", "abc"));
    }
}
