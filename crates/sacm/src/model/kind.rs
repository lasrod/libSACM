//! Element kind metadata: the concrete SACM classes, the abstract classes above
//! them and the metamodel packages they belong to.

use std::fmt;

/// One of the five packages of the SACM 2.3 metamodel.
///
/// Every SACM class, concrete or abstract, is defined in exactly one package.
/// Each package is a clause of the specification: `Base` (clause 8),
/// `AssuranceCase` (clause 9), `Terminology` (clause 10), `Argumentation`
/// (clause 11) and `Artifact` (clause 12).
///
/// The [`Ord`] order is the order of the clauses today. It is not a stability
/// promise, and new variants may be added in a minor release.
///
/// # Examples
///
/// ```
/// use sacm::model::{ElementKind, MetamodelPackage};
///
/// assert_eq!(ElementKind::Claim.package(), MetamodelPackage::Argumentation);
/// assert_eq!(MetamodelPackage::Argumentation.to_string(), "Argumentation");
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetamodelPackage {
    /// `Base` (clause 8): the classes every other package builds on.
    Base,
    /// `AssuranceCase` (clause 9): the assurance case package, the unit of interchange.
    AssuranceCase,
    /// `Terminology` (clause 10): terms, expressions and categories.
    Terminology,
    /// `Argumentation` (clause 11): claims, reasoning and asserted relationships.
    Argumentation,
    /// `Artifact` (clause 12): artifacts, the activities and events around them.
    Artifact,
}

impl MetamodelPackage {
    /// The package's name, spelled as in the SACM 2.3 metamodel.
    ///
    /// ```
    /// use sacm::model::MetamodelPackage;
    ///
    /// assert_eq!(MetamodelPackage::AssuranceCase.name(), "AssuranceCase");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Base => "Base",
            Self::AssuranceCase => "AssuranceCase",
            Self::Terminology => "Terminology",
            Self::Argumentation => "Argumentation",
            Self::Artifact => "Artifact",
        }
    }
}

impl fmt::Display for MetamodelPackage {
    /// Writes [`MetamodelPackage::name`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// An abstract class of the SACM 2.3 metamodel.
///
/// Abstract classes have no instances of their own. They group the concrete
/// classes ([`ElementKind`]) that share their features, and
/// [`ElementKind::is_a`] answers whether a kind is one of them. `Element`, the
/// root of every generalization chain, is included.
///
/// The [`Ord`] order is not a stability promise, [`AbstractClass::ALL`] may
/// grow, and new variants may be added in a minor release.
///
/// # Examples
///
/// ```
/// use sacm::model::{AbstractClass, ElementKind, MetamodelPackage};
///
/// let class = AbstractClass::AssertedRelationship;
/// assert_eq!(class.package(), MetamodelPackage::Argumentation);
/// assert!(ElementKind::AssertedInference.is_a(class));
/// assert!(!ElementKind::Claim.is_a(class));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractClass {
    // Base (clause 8)
    /// `Element`: the root of every generalization chain.
    Element,
    /// `SACMElement`: every element that can appear in an assurance case.
    SACMElement,
    /// `ModelElement`: a named element of the model proper.
    ModelElement,
    /// `UtilityElement`: descriptions, notes, tagged values and implementation constraints.
    UtilityElement,
    /// `ArtifactElement`: anything a package can contain, packages included.
    ArtifactElement,
    // Terminology (clause 10)
    /// `TerminologyElement`: anything in a terminology package.
    TerminologyElement,
    /// `TerminologyAsset`: categories and expression elements.
    TerminologyAsset,
    /// `ExpressionElement`: expressions and terms.
    ExpressionElement,
    // Argumentation (clause 11)
    /// `ArgumentationElement`: anything in an argument package.
    ArgumentationElement,
    /// `ArgumentAsset`: assertions, reasoning and artifact references.
    ArgumentAsset,
    /// `Assertion`: claims and asserted relationships.
    Assertion,
    /// `AssertedRelationship`: asserted inferences, evidence, context and artifact links.
    AssertedRelationship,
    // Artifact (clause 12)
    /// `ArtifactAsset`: artifacts, activities, events, participants, techniques,
    /// resources, properties and the relationships between them.
    ArtifactAsset,
}

impl AbstractClass {
    /// Every abstract class, in the metamodel's package order.
    ///
    /// The list may grow in a minor release.
    ///
    /// ```
    /// use sacm::model::AbstractClass;
    ///
    /// assert!(AbstractClass::ALL.contains(&AbstractClass::Assertion));
    /// ```
    pub const ALL: &[AbstractClass] = &[
        Self::Element,
        Self::SACMElement,
        Self::ModelElement,
        Self::UtilityElement,
        Self::ArtifactElement,
        Self::TerminologyElement,
        Self::TerminologyAsset,
        Self::ExpressionElement,
        Self::ArgumentationElement,
        Self::ArgumentAsset,
        Self::Assertion,
        Self::AssertedRelationship,
        Self::ArtifactAsset,
    ];

    /// The class's name, spelled as in the SACM 2.3 metamodel.
    ///
    /// ```
    /// use sacm::model::AbstractClass;
    ///
    /// assert_eq!(AbstractClass::SACMElement.name(), "SACMElement");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Element => "Element",
            Self::SACMElement => "SACMElement",
            Self::ModelElement => "ModelElement",
            Self::UtilityElement => "UtilityElement",
            Self::ArtifactElement => "ArtifactElement",
            Self::TerminologyElement => "TerminologyElement",
            Self::TerminologyAsset => "TerminologyAsset",
            Self::ExpressionElement => "ExpressionElement",
            Self::ArgumentationElement => "ArgumentationElement",
            Self::ArgumentAsset => "ArgumentAsset",
            Self::Assertion => "Assertion",
            Self::AssertedRelationship => "AssertedRelationship",
            Self::ArtifactAsset => "ArtifactAsset",
        }
    }

    /// The package that defines the class.
    ///
    /// ```
    /// use sacm::model::{AbstractClass, MetamodelPackage};
    ///
    /// assert_eq!(AbstractClass::ArtifactElement.package(), MetamodelPackage::Base);
    /// ```
    #[must_use]
    pub const fn package(self) -> MetamodelPackage {
        match self {
            Self::Element
            | Self::SACMElement
            | Self::ModelElement
            | Self::UtilityElement
            | Self::ArtifactElement => MetamodelPackage::Base,
            Self::TerminologyElement | Self::TerminologyAsset | Self::ExpressionElement => {
                MetamodelPackage::Terminology
            }
            Self::ArgumentationElement
            | Self::ArgumentAsset
            | Self::Assertion
            | Self::AssertedRelationship => MetamodelPackage::Argumentation,
            Self::ArtifactAsset => MetamodelPackage::Artifact,
        }
    }

    /// The class's direct superclass, or `None` for `Element`.
    const fn superclass(self) -> Option<Self> {
        match self {
            Self::Element => None,
            Self::SACMElement => Some(Self::Element),
            Self::ModelElement | Self::UtilityElement => Some(Self::SACMElement),
            Self::ArtifactElement => Some(Self::ModelElement),
            Self::TerminologyElement | Self::ArgumentationElement | Self::ArtifactAsset => {
                Some(Self::ArtifactElement)
            }
            Self::TerminologyAsset => Some(Self::TerminologyElement),
            Self::ExpressionElement => Some(Self::TerminologyAsset),
            Self::ArgumentAsset => Some(Self::ArgumentationElement),
            Self::Assertion => Some(Self::ArgumentAsset),
            Self::AssertedRelationship => Some(Self::Assertion),
        }
    }

    /// Whether `self` and `other` are the same class, usable in `const fn`.
    const fn same(self, other: Self) -> bool {
        self as u8 == other as u8
    }
}

impl fmt::Display for AbstractClass {
    /// Writes [`AbstractClass::name`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// The kind of an element: one of the concrete `SACMElement` classes of SACM 2.3.
///
/// Every element of a [`Document`](super::Document) has exactly one kind. The
/// variants are named, and [`name`](Self::name) spells them, exactly as the
/// SACM classes. The value types `LangString`, `MultiLangString` and
/// `ExpressionLangString` are not `SACMElement`s and have no kind.
///
/// The [`Ord`] order is the order of the metamodel's packages today. It is not
/// a stability promise, [`ElementKind::ALL`] may grow, and new variants may be
/// added in a minor release.
///
/// # Examples
///
/// ```
/// use sacm::model::{AbstractClass, ElementKind, MetamodelPackage};
///
/// let kind = ElementKind::AssertedEvidence;
/// assert_eq!(kind.name(), "AssertedEvidence");
/// assert_eq!(kind.package(), MetamodelPackage::Argumentation);
/// assert!(kind.is_a(AbstractClass::Assertion));
/// assert!(!kind.is_package());
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementKind {
    // Base (clause 8)
    /// `Description`: a utility element describing another element.
    Description,
    /// `ImplementationConstraint`: a constraint on how an element is implemented.
    ImplementationConstraint,
    /// `Note`: a free-form note attached to an element.
    Note,
    /// `TaggedValue`: a key and value attached to an element.
    TaggedValue,
    // AssuranceCase (clause 9)
    /// `AssuranceCasePackage`: an assurance case, the unit of interchange.
    AssuranceCasePackage,
    /// `AssuranceCasePackageInterface`: the declared interface of an assurance case package.
    AssuranceCasePackageInterface,
    /// `AssuranceCasePackageBinding`: a binding between assurance case packages.
    AssuranceCasePackageBinding,
    // Terminology (clause 10)
    /// `TerminologyPackage`: a container of terms, expressions and categories.
    TerminologyPackage,
    /// `TerminologyPackageInterface`: the declared interface of a terminology package.
    TerminologyPackageInterface,
    /// `TerminologyPackageBinding`: a binding between terminology packages.
    TerminologyPackageBinding,
    /// `TerminologyGroup`: a grouping of terminology elements.
    TerminologyGroup,
    /// `Category`: a label that classifies terms and expressions.
    Category,
    /// `Expression`: an expression built from terms.
    Expression,
    /// `Term`: a defined word or phrase, from which expressions are built.
    Term,
    // Argumentation (clause 11)
    /// `ArgumentPackage`: a container of claims, reasoning and asserted relationships.
    ArgumentPackage,
    /// `ArgumentPackageInterface`: the declared interface of an argument package.
    ArgumentPackageInterface,
    /// `ArgumentPackageBinding`: a binding between argument packages.
    ArgumentPackageBinding,
    /// `ArgumentGroup`: a grouping of argumentation elements.
    ArgumentGroup,
    /// `Claim`: a proposition put forward as true.
    Claim,
    /// `ArgumentReasoning`: the reasoning behind an asserted inference.
    ArgumentReasoning,
    /// `ArtifactReference`: a reference from an argument to an artifact.
    ArtifactReference,
    /// `AssertedInference`: an assertion that premises support a conclusion.
    AssertedInference,
    /// `AssertedEvidence`: an assertion that evidence supports a claim.
    AssertedEvidence,
    /// `AssertedContext`: an assertion that an element is context for another.
    AssertedContext,
    /// `AssertedArtifactSupport`: an assertion that an artifact supports another.
    AssertedArtifactSupport,
    /// `AssertedArtifactContext`: an assertion that an artifact is context for another.
    AssertedArtifactContext,
    // Artifact (clause 12)
    /// `ArtifactPackage`: a container of artifacts and the activities, events and
    /// relationships around them.
    ArtifactPackage,
    /// `ArtifactPackageInterface`: the declared interface of an artifact package.
    ArtifactPackageInterface,
    /// `ArtifactPackageBinding`: a binding between artifact packages.
    ArtifactPackageBinding,
    /// `ArtifactGroup`: a grouping of artifact elements.
    ArtifactGroup,
    /// `Artifact`: a work product, such as a document, a test report or a model.
    Artifact,
    /// `ArtifactAssetRelationship`: a relationship between artifact assets
    /// (clause 12.14; the machine-readable model misnames it
    /// `ArtifactAssertedRelationship`).
    ArtifactAssetRelationship,
    /// `Activity`: an activity that produces or uses artifacts.
    Activity,
    /// `Event`: an event in the life of an artifact.
    Event,
    /// `Participant`: a person or organisation taking part in an activity.
    Participant,
    /// `Technique`: a technique used in an activity.
    Technique,
    /// `Resource`: a resource used in an activity.
    Resource,
    /// `Property`: a property of an artifact asset.
    Property,
}

impl ElementKind {
    /// Every element kind, in the metamodel's package order.
    ///
    /// The list may grow in a minor release.
    ///
    /// ```
    /// use sacm::model::ElementKind;
    ///
    /// let packages = ElementKind::ALL.iter().filter(|kind| kind.is_package());
    /// assert_eq!(packages.count(), 12);
    /// ```
    pub const ALL: &[ElementKind] = &[
        Self::Description,
        Self::ImplementationConstraint,
        Self::Note,
        Self::TaggedValue,
        Self::AssuranceCasePackage,
        Self::AssuranceCasePackageInterface,
        Self::AssuranceCasePackageBinding,
        Self::TerminologyPackage,
        Self::TerminologyPackageInterface,
        Self::TerminologyPackageBinding,
        Self::TerminologyGroup,
        Self::Category,
        Self::Expression,
        Self::Term,
        Self::ArgumentPackage,
        Self::ArgumentPackageInterface,
        Self::ArgumentPackageBinding,
        Self::ArgumentGroup,
        Self::Claim,
        Self::ArgumentReasoning,
        Self::ArtifactReference,
        Self::AssertedInference,
        Self::AssertedEvidence,
        Self::AssertedContext,
        Self::AssertedArtifactSupport,
        Self::AssertedArtifactContext,
        Self::ArtifactPackage,
        Self::ArtifactPackageInterface,
        Self::ArtifactPackageBinding,
        Self::ArtifactGroup,
        Self::Artifact,
        Self::ArtifactAssetRelationship,
        Self::Activity,
        Self::Event,
        Self::Participant,
        Self::Technique,
        Self::Resource,
        Self::Property,
    ];

    /// The kind's class name, spelled exactly as in SACM 2.3.
    ///
    /// ```
    /// use sacm::model::ElementKind;
    ///
    /// assert_eq!(ElementKind::ArtifactAssetRelationship.name(), "ArtifactAssetRelationship");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::ImplementationConstraint => "ImplementationConstraint",
            Self::Note => "Note",
            Self::TaggedValue => "TaggedValue",
            Self::AssuranceCasePackage => "AssuranceCasePackage",
            Self::AssuranceCasePackageInterface => "AssuranceCasePackageInterface",
            Self::AssuranceCasePackageBinding => "AssuranceCasePackageBinding",
            Self::TerminologyPackage => "TerminologyPackage",
            Self::TerminologyPackageInterface => "TerminologyPackageInterface",
            Self::TerminologyPackageBinding => "TerminologyPackageBinding",
            Self::TerminologyGroup => "TerminologyGroup",
            Self::Category => "Category",
            Self::Expression => "Expression",
            Self::Term => "Term",
            Self::ArgumentPackage => "ArgumentPackage",
            Self::ArgumentPackageInterface => "ArgumentPackageInterface",
            Self::ArgumentPackageBinding => "ArgumentPackageBinding",
            Self::ArgumentGroup => "ArgumentGroup",
            Self::Claim => "Claim",
            Self::ArgumentReasoning => "ArgumentReasoning",
            Self::ArtifactReference => "ArtifactReference",
            Self::AssertedInference => "AssertedInference",
            Self::AssertedEvidence => "AssertedEvidence",
            Self::AssertedContext => "AssertedContext",
            Self::AssertedArtifactSupport => "AssertedArtifactSupport",
            Self::AssertedArtifactContext => "AssertedArtifactContext",
            Self::ArtifactPackage => "ArtifactPackage",
            Self::ArtifactPackageInterface => "ArtifactPackageInterface",
            Self::ArtifactPackageBinding => "ArtifactPackageBinding",
            Self::ArtifactGroup => "ArtifactGroup",
            Self::Artifact => "Artifact",
            Self::ArtifactAssetRelationship => "ArtifactAssetRelationship",
            Self::Activity => "Activity",
            Self::Event => "Event",
            Self::Participant => "Participant",
            Self::Technique => "Technique",
            Self::Resource => "Resource",
            Self::Property => "Property",
        }
    }

    /// The package that defines the kind's class.
    ///
    /// ```
    /// use sacm::model::{ElementKind, MetamodelPackage};
    ///
    /// assert_eq!(ElementKind::Term.package(), MetamodelPackage::Terminology);
    /// assert_eq!(ElementKind::Note.package(), MetamodelPackage::Base);
    /// ```
    #[must_use]
    pub const fn package(self) -> MetamodelPackage {
        match self {
            Self::Description | Self::ImplementationConstraint | Self::Note | Self::TaggedValue => {
                MetamodelPackage::Base
            }
            Self::AssuranceCasePackage
            | Self::AssuranceCasePackageInterface
            | Self::AssuranceCasePackageBinding => MetamodelPackage::AssuranceCase,
            Self::TerminologyPackage
            | Self::TerminologyPackageInterface
            | Self::TerminologyPackageBinding
            | Self::TerminologyGroup
            | Self::Category
            | Self::Expression
            | Self::Term => MetamodelPackage::Terminology,
            Self::ArgumentPackage
            | Self::ArgumentPackageInterface
            | Self::ArgumentPackageBinding
            | Self::ArgumentGroup
            | Self::Claim
            | Self::ArgumentReasoning
            | Self::ArtifactReference
            | Self::AssertedInference
            | Self::AssertedEvidence
            | Self::AssertedContext
            | Self::AssertedArtifactSupport
            | Self::AssertedArtifactContext => MetamodelPackage::Argumentation,
            Self::ArtifactPackage
            | Self::ArtifactPackageInterface
            | Self::ArtifactPackageBinding
            | Self::ArtifactGroup
            | Self::Artifact
            | Self::ArtifactAssetRelationship
            | Self::Activity
            | Self::Event
            | Self::Participant
            | Self::Technique
            | Self::Resource
            | Self::Property => MetamodelPackage::Artifact,
        }
    }

    /// Whether the kind's class specializes `class`: `class` is in its
    /// generalization chain.
    ///
    /// ```
    /// use sacm::model::{AbstractClass, ElementKind};
    ///
    /// assert!(ElementKind::Claim.is_a(AbstractClass::Assertion));
    /// assert!(ElementKind::Claim.is_a(AbstractClass::Element));
    /// assert!(!ElementKind::Claim.is_a(AbstractClass::AssertedRelationship));
    /// ```
    #[must_use]
    pub const fn is_a(self, class: AbstractClass) -> bool {
        let mut current = Some(self.abstract_superclass());
        while let Some(superclass) = current {
            if superclass.same(class) {
                return true;
            }
            current = superclass.superclass();
        }
        false
    }

    /// Whether the kind is `other`, or specializes it.
    ///
    /// Among concrete classes, only the interface and binding of each package
    /// specialize another kind: their package.
    ///
    /// ```
    /// use sacm::model::ElementKind;
    ///
    /// assert!(ElementKind::Claim.is_kind_of(ElementKind::Claim));
    /// assert!(ElementKind::ArgumentPackageBinding.is_kind_of(ElementKind::ArgumentPackage));
    /// assert!(!ElementKind::ArgumentPackage.is_kind_of(ElementKind::ArgumentPackageBinding));
    /// ```
    #[must_use]
    pub const fn is_kind_of(self, other: ElementKind) -> bool {
        if self.same(other) {
            return true;
        }
        match self.concrete_superclass() {
            Some(superclass) => superclass.same(other),
            None => false,
        }
    }

    /// Whether the kind is a package: an assurance case, terminology, argument
    /// or artifact package, or the interface or binding of one.
    ///
    /// ```
    /// use sacm::model::ElementKind;
    ///
    /// assert!(ElementKind::AssuranceCasePackage.is_package());
    /// assert!(ElementKind::TerminologyPackageInterface.is_package());
    /// assert!(!ElementKind::ArgumentGroup.is_package());
    /// ```
    #[must_use]
    pub const fn is_package(self) -> bool {
        matches!(
            self,
            Self::AssuranceCasePackage
                | Self::AssuranceCasePackageInterface
                | Self::AssuranceCasePackageBinding
                | Self::TerminologyPackage
                | Self::TerminologyPackageInterface
                | Self::TerminologyPackageBinding
                | Self::ArgumentPackage
                | Self::ArgumentPackageInterface
                | Self::ArgumentPackageBinding
                | Self::ArtifactPackage
                | Self::ArtifactPackageInterface
                | Self::ArtifactPackageBinding
        )
    }

    /// The concrete class this kind's class specializes, if any: the package of
    /// a package interface or binding.
    const fn concrete_superclass(self) -> Option<Self> {
        match self {
            Self::AssuranceCasePackageInterface | Self::AssuranceCasePackageBinding => {
                Some(Self::AssuranceCasePackage)
            }
            Self::TerminologyPackageInterface | Self::TerminologyPackageBinding => {
                Some(Self::TerminologyPackage)
            }
            Self::ArgumentPackageInterface | Self::ArgumentPackageBinding => {
                Some(Self::ArgumentPackage)
            }
            Self::ArtifactPackageInterface | Self::ArtifactPackageBinding => {
                Some(Self::ArtifactPackage)
            }
            _ => None,
        }
    }

    /// The nearest abstract class in the kind's generalization chain.
    const fn abstract_superclass(self) -> AbstractClass {
        match self {
            Self::Description | Self::ImplementationConstraint | Self::Note | Self::TaggedValue => {
                AbstractClass::UtilityElement
            }
            Self::AssuranceCasePackage
            | Self::AssuranceCasePackageInterface
            | Self::AssuranceCasePackageBinding
            | Self::ArtifactPackage
            | Self::ArtifactPackageInterface
            | Self::ArtifactPackageBinding
            | Self::ArtifactGroup => AbstractClass::ArtifactElement,
            Self::TerminologyPackage
            | Self::TerminologyPackageInterface
            | Self::TerminologyPackageBinding
            | Self::TerminologyGroup => AbstractClass::TerminologyElement,
            Self::Category => AbstractClass::TerminologyAsset,
            Self::Expression | Self::Term => AbstractClass::ExpressionElement,
            Self::ArgumentPackage
            | Self::ArgumentPackageInterface
            | Self::ArgumentPackageBinding
            | Self::ArgumentGroup => AbstractClass::ArgumentationElement,
            Self::Claim => AbstractClass::Assertion,
            Self::ArgumentReasoning | Self::ArtifactReference => AbstractClass::ArgumentAsset,
            Self::AssertedInference
            | Self::AssertedEvidence
            | Self::AssertedContext
            | Self::AssertedArtifactSupport
            | Self::AssertedArtifactContext => AbstractClass::AssertedRelationship,
            Self::Artifact
            | Self::ArtifactAssetRelationship
            | Self::Activity
            | Self::Event
            | Self::Participant
            | Self::Technique
            | Self::Resource
            | Self::Property => AbstractClass::ArtifactAsset,
        }
    }

    /// Whether `self` and `other` are the same kind, usable in `const fn`.
    const fn same(self, other: Self) -> bool {
        self as u8 == other as u8
    }
}

impl fmt::Display for ElementKind {
    /// Writes [`ElementKind::name`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::{AbstractClass, ElementKind};

    #[test]
    fn element_kind_all_is_in_declaration_order() {
        for (position, kind) in ElementKind::ALL.iter().enumerate() {
            assert_eq!(*kind as usize, position, "{kind} is out of place in ALL");
        }
    }

    #[test]
    fn abstract_class_all_is_in_declaration_order() {
        for (position, class) in AbstractClass::ALL.iter().enumerate() {
            assert_eq!(*class as usize, position, "{class} is out of place in ALL");
        }
    }
}
