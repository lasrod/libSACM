//! Property tests of the round-trip and edit guarantees (LIB-RT-004; D-RT,
//! D-ATOMIC, ADR 0032; ADR 0046 D1).
//!
//! Each property draws a list of [`Step`]s with proptest and decodes each step
//! into an [`Operation`] against the document it is applied to, so the
//! sequences reach every operation, valid and invalid ids, colliding caller
//! ids and text with every XML 1.0 `Char` range plus characters it cannot
//! represent (LIB-EDIT-025). The six properties:
//!
//! 1. A built document writes and reads back `==` in both modes, and writes
//!    the same bytes every time.
//! 2. A refused edit changes nothing: the document, its revision and its next
//!    generated ids; `edit::check` gives the same error.
//! 3. `apply_all` is all or nothing, and names the first refusal.
//! 4. A batch whose closure returns `Err` changes nothing.
//! 5. A committed batch's net ids are those of `compare::changes`.
//! 6. A document from a strict read, after any edits `edit::apply` accepted,
//!    writes strictly and reads back `==` (the `xmi::write` guarantee).
//!
//! # Reproducibility
//!
//! Each property has a fixed case count and seed ([`config`]), so every run
//! draws the same cases. `PROPTEST_CASES` and `PROPTEST_RNG_SEED` override
//! them (the weekly run and the mutation jobs). Failures are not persisted to
//! files: a found case becomes a `lib_rt_004_regression_<n>_<what>` test with
//! the literal steps.

use std::convert::Infallible;
use std::fmt::Debug;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use proptest::collection::vec;
use proptest::option;
use proptest::prelude::{ProptestConfig, Strategy, any, prop_assert, prop_assert_eq, prop_oneof};
use proptest::sample::select;
use proptest::test_runner::{RngSeed, TestCaseError, TestRunner};
use sacm::compare::{changes, semantic_compare};
use sacm::edit::{
    self, AddReference, CopyElements, CreateActivity, CreateArgumentGroup, CreateArgumentPackage,
    CreateArgumentPackageBinding, CreateArgumentPackageInterface, CreateArgumentReasoning,
    CreateArtifact, CreateArtifactAssetRelationship, CreateArtifactGroup, CreateArtifactPackage,
    CreateArtifactPackageBinding, CreateArtifactPackageInterface, CreateArtifactReference,
    CreateAssertedArtifactContext, CreateAssertedArtifactSupport, CreateAssertedContext,
    CreateAssertedEvidence, CreateAssertedInference, CreateAssuranceCasePackage,
    CreateAssuranceCasePackageBinding, CreateAssuranceCasePackageInterface, CreateCategory,
    CreateClaim, CreateDescription, CreateEvent, CreateExpression, CreateImplementationConstraint,
    CreateNote, CreateParticipant, CreateProperty, CreateResource, CreateTaggedValue,
    CreateTechnique, CreateTerm, CreateTerminologyGroup, CreateTerminologyPackage,
    CreateTerminologyPackageBinding, CreateTerminologyPackageInterface,
    CrossPackageReferencePolicy, DeleteElement, EditError, MoveElement, Mutation, Operation,
    PackageDeletePolicy, ReferenceDeletePolicy, RemoveReference, ReorderElements,
    SetAssertionDeclaration, SetContent, SetDate, SetEndTime, SetExternalReference, SetFlag,
    SetGid, SetKey, SetLocation, SetName, SetReferences, SetStartTime, SetTaggedValue, SetValue,
    SetVersion,
};
use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, Flag, Href, LangString,
    ReferenceEnd,
};
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const RT_004: &str = "LIB-RT-004";

/// The id [`pick`] gives for the slot after the last element: no document
/// here holds it.
const NOWHERE: &str = "nowhere";

/// The href of the `new_href` forms: an element of another document.
const HREF: &str = "other.sacm.xmi#x";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The [`Operation`] variants in declaration order: `op % OPERATIONS.len()`
/// selects one. [`assert_the_decoder_covers_every_operation`] checks the list
/// against the enum's declaration and against what [`operation`] builds.
const OPERATIONS: [&str; 59] = [
    "CreateAssuranceCasePackage",
    "CreateAssuranceCasePackageInterface",
    "CreateAssuranceCasePackageBinding",
    "CreateArgumentPackage",
    "CreateArgumentPackageInterface",
    "CreateArgumentPackageBinding",
    "CreateClaim",
    "CreateArgumentReasoning",
    "CreateAssertedInference",
    "CreateAssertedContext",
    "CreateArtifactReference",
    "CreateArgumentGroup",
    "CreateAssertedEvidence",
    "CreateAssertedArtifactSupport",
    "CreateAssertedArtifactContext",
    "CreateDescription",
    "CreateImplementationConstraint",
    "CreateNote",
    "CreateTaggedValue",
    "CreateArtifactPackage",
    "CreateArtifactPackageInterface",
    "CreateArtifactPackageBinding",
    "CreateProperty",
    "CreateTechnique",
    "CreateParticipant",
    "CreateArtifactGroup",
    "CreateArtifact",
    "CreateEvent",
    "CreateResource",
    "CreateActivity",
    "CreateArtifactAssetRelationship",
    "CreateTerminologyPackage",
    "CreateTerminologyPackageInterface",
    "CreateTerminologyPackageBinding",
    "CreateCategory",
    "CreateTerm",
    "CreateExpression",
    "CreateTerminologyGroup",
    "SetName",
    "SetGid",
    "SetFlag",
    "SetAssertionDeclaration",
    "SetContent",
    "SetKey",
    "SetTaggedValue",
    "SetVersion",
    "SetDate",
    "SetLocation",
    "SetStartTime",
    "SetEndTime",
    "SetValue",
    "SetExternalReference",
    "SetReferences",
    "AddReference",
    "RemoveReference",
    "ReorderElements",
    "MoveElement",
    "CopyElements",
    "DeleteElement",
];

/// The fixtures property 6 starts from, under `tests/data/sacm23`.
const STRICT_FIXTURES: [&str; 6] = [
    "cp/argumentation-unit.sacm.xmi",
    "cp/artifact-unit.sacm.xmi",
    "cp/assurance-case-unit.sacm.xmi",
    "cp/terminology-unit.sacm.xmi",
    "xmi/xmi-metadata-valid.sacm.xmi",
    "xmi/difference-target-ids-valid.sacm.xmi",
];

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

/// One drawn edit, decoded into an [`Operation`] by [`operation`] against the
/// document it is applied to.
#[derive(Clone, Debug)]
struct Step {
    /// Selects the operation: `op % OPERATIONS.len()`.
    op: u8,
    /// The parent, or the element edited: `pick(a)`, or for a biased step
    /// the `a`-th of the elements of an admitted kind ([`biased_kinds`]).
    a: u16,
    /// The other element, such as a target or a reference: `pick(b)`, or
    /// biased as `a` is.
    b: u16,
    /// The lists of ids, such as the sources: each `pick(n)`, or biased as
    /// `a` is; empty means `[other]`.
    picks: Vec<u16>,
    /// The name's content, and every other text.
    text: String,
    /// The language, root or child, set or remove, flag, declaration, end,
    /// policies and href, each by modulo; and, from 32, a biased step
    /// ([`is_biased`]).
    choice: u8,
    /// The caller id `x{n}` of a create, or a generated id.
    id: Option<u8>,
}

/// The configuration of a property: `cases` cases from the fixed `seed`,
/// unless `PROPTEST_CASES` or `PROPTEST_RNG_SEED` is set, and no failure
/// files (ADR 0046 D1).
fn config(cases: u32, seed: u64) -> ProptestConfig {
    let mut config = ProptestConfig::default();
    if std::env::var_os("PROPTEST_CASES").is_none() {
        config.cases = cases;
    }
    if std::env::var_os("PROPTEST_RNG_SEED").is_none() {
        config.rng_seed = RngSeed::Fixed(seed);
    }
    config.failure_persistence = None;
    config
}

/// Runs `test` over `strategy` under [`config`]`(cases, seed)`, failing with
/// the shrunk case.
fn run<S>(cases: u32, seed: u64, strategy: &S, test: impl Fn(S::Value) -> Result<(), TestCaseError>)
where
    S: Strategy,
    S::Value: Debug,
{
    let mut runner = TestRunner::new(config(cases, seed));
    if let Err(error) = runner.run(strategy, test) {
        panic!("{RT_004}: {error}");
    }
}

/// 0 to 12 characters from weighted parts: XML-significant ASCII, the white
/// space XML 1.0 allows, the three ranges of the BMP and the supplementary
/// planes XML 1.0's `Char` allows, and U+0001..U+0008, which it does not.
fn text() -> impl Strategy<Value = String> {
    let part = prop_oneof![
        6 => select(vec![
            'a', 'Z', '0', '_', '-', '.', ' ', '<', '>', '&', '"', '\'', '#', ':',
        ]),
        1 => select(vec!['\t', '\n', '\r']),
        2 => proptest::char::range('\u{80}', '\u{D7FF}'),
        1 => proptest::char::range('\u{E000}', '\u{FFFD}'),
        1 => proptest::char::range('\u{10000}', '\u{10FFFF}'),
        1 => proptest::char::range('\u{1}', '\u{8}'),
    ];
    vec(part, 0..=12).prop_map(|chars| chars.into_iter().collect())
}

fn step() -> impl Strategy<Value = Step> {
    (
        any::<u8>(),
        any::<u16>(),
        any::<u16>(),
        vec(any::<u16>(), 0..=3),
        text(),
        any::<u8>(),
        option::of(0u8..32),
    )
        .prop_map(|(op, a, b, picks, text, choice, id)| Step {
            op,
            a,
            b,
            picks,
            text,
            choice,
            id,
        })
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    vec(step(), 0..=40)
}

/// The id at `n % (len + 1)` of the pre-order `ids`; the last slot is
/// [`NOWHERE`].
fn pick(ids: &[ElementId], n: u16) -> ElementId {
    ids.get(usize::from(n) % (ids.len() + 1))
        .cloned()
        .unwrap_or_else(|| ElementId::from(NOWHERE))
}

/// The kinds a biased pick draws among.
#[derive(Clone, Copy)]
enum Kinds {
    /// The kinds that may contain an element of this kind: a parent.
    ParentOf(ElementKind),
    /// This kind or a kind that specializes it, such as a package's interface.
    KindOf(ElementKind),
    /// This kind only.
    Exactly(ElementKind),
    /// The kinds this test accepts.
    Where(fn(ElementKind) -> bool),
    /// The kinds the first test accepts, or, when the document holds none,
    /// those the second accepts.
    Preferring(fn(ElementKind) -> bool, fn(ElementKind) -> bool),
}

/// The id at `n % len` of the ids in `elements` whose kind `admits` accepts,
/// or `None` when there is none.
fn among_where(
    elements: &[(ElementId, ElementKind)],
    n: u16,
    admits: impl Fn(ElementKind) -> bool,
) -> Option<ElementId> {
    let pool: Vec<&ElementId> = elements
        .iter()
        .filter(|(_, kind)| admits(*kind))
        .map(|(id, _)| id)
        .collect();
    let at = usize::from(n).checked_rem(pool.len())?;
    pool.get(at).map(|&id| id.clone())
}

/// The id at `n % len` of the ids in `elements` of the kinds `kinds` draws
/// among, or `None` when there is none.
fn among(elements: &[(ElementId, ElementKind)], n: u16, kinds: Kinds) -> Option<ElementId> {
    match kinds {
        Kinds::ParentOf(child) => among_where(elements, n, |kind| kind.may_contain(child)),
        Kinds::KindOf(class) => among_where(elements, n, |kind| kind.is_kind_of(class)),
        Kinds::Exactly(only) => among_where(elements, n, |kind| kind == only),
        Kinds::Where(test) => among_where(elements, n, test),
        Kinds::Preferring(first, then) => {
            among_where(elements, n, first).or_else(|| among_where(elements, n, then))
        }
    }
}

/// Whether `step` draws its ids among the kinds its operation admits, seven
/// steps in eight, rather than among every id: so creates find a parent, sets
/// find an element that has the feature, and references find their kinds.
fn is_biased(step: &Step) -> bool {
    step.choice >= 32
}

/// The element kind named `name`.
fn kind_named(name: &str) -> Option<ElementKind> {
    ElementKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.name() == name)
}

/// The kind of element the operation at `index` creates: the name of its
/// `Create*` struct is the kind's name.
fn created_kind(index: usize) -> Option<ElementKind> {
    kind_named(OPERATIONS.get(index)?.strip_prefix("Create")?)
}

/// The package kind of the interface or binding the operation at `index`
/// creates: an `ArgumentPackage` for an `ArgumentPackageBinding`.
fn package_family(index: usize) -> Option<ElementKind> {
    let kind = OPERATIONS.get(index)?.strip_prefix("Create")?;
    kind_named(
        kind.strip_suffix("Interface")
            .or_else(|| kind.strip_suffix("Binding"))?,
    )
}

/// Whether `kind` is an `ArgumentAsset`: an assertion, an
/// `ArgumentReasoning` or an `ArtifactReference`.
fn is_argument_asset(kind: ElementKind) -> bool {
    kind.is_a(AbstractClass::ArgumentAsset)
}

fn is_artifact_reference(kind: ElementKind) -> bool {
    kind == ElementKind::ArtifactReference
}

/// The ends of a biased step: what its element, its sources and its target
/// are drawn among, each `None` for every id.
type Ends = (Option<Kinds>, Option<Kinds>, Option<Kinds>);

/// What a biased step of the operation at `index` draws among: its element
/// (the parent of a create, or the element a set or reference edit edits),
/// its sources (each id of a list) and its target (the other id). `end` and
/// `flag` are the step's.
///
/// - **A create** draws its parent among the elements that may contain the
///   kind it creates.
/// - **An asserted relationship** (SACM 2.3 §11.14 to §11.18) draws its
///   sources and target among `ArgumentAsset`s, or among `ArtifactReference`s
///   for the sources of an `AssertedEvidence` and both ends of an
///   `AssertedArtifactSupport` or `AssertedArtifactContext` (any
///   `ArgumentAsset` while the document holds no `ArtifactReference`).
/// - **An `ArtifactAssetRelationship`** (§12.14) relates `ArtifactAsset`s.
/// - **An interface** implements a package of its family, and **a binding**
///   binds packages of its family, interfaces included.
/// - **A set** edits an element that has the feature: a `ModelElement` for a
///   name or a tagged value, an Assertion for a declaration, a
///   `UtilityElement` for a content, a `TaggedValue` for a key, an `Artifact`
///   for a version, an `Artifact` or `Event` for a date, a `Resource` for a
///   location, an `Activity` for a start or end time, a `Term` or
///   `Expression` for a value, a `Term` for an external reference, and an
///   asserted relationship for `isCounter`.
/// - **A reference edit** of the `source`, `target` or `reasoning` end edits an
///   asserted relationship, with ids of the kinds the end admits;
///   [`held_reference`] gives a remove a reference the document holds.
fn biased_kinds(index: usize, end: ReferenceEnd, flag: Flag) -> Ends {
    let parent = created_kind(index).map(Kinds::ParentOf);
    let asset = Some(Kinds::Where(is_argument_asset));
    // A create accepts any `ArgumentAsset` where §11.15, §11.17 and §11.18
    // name an `ArtifactReference`, so one stands in while there is none.
    let reference = Some(Kinds::Preferring(is_artifact_reference, is_argument_asset));
    let relationship = Some(Kinds::Where(|kind| {
        kind.is_a(AbstractClass::AssertedRelationship)
    }));
    let element = |test: fn(ElementKind) -> bool| (Some(Kinds::Where(test)), None, None);
    let operation = OPERATIONS.get(index).copied().unwrap_or_default();
    match operation {
        "CreateAssertedInference" | "CreateAssertedContext" => (parent, asset, asset),
        "CreateAssertedEvidence" => (parent, reference, asset),
        "CreateAssertedArtifactSupport" | "CreateAssertedArtifactContext" => {
            (parent, reference, reference)
        }
        "CreateArtifactAssetRelationship" => {
            let artifact_asset = Some(Kinds::Where(|kind| kind.is_a(AbstractClass::ArtifactAsset)));
            (parent, artifact_asset, artifact_asset)
        }
        _ if operation.ends_with("Interface") => {
            (parent, None, package_family(index).map(Kinds::Exactly))
        }
        _ if operation.ends_with("Binding") => {
            (parent, package_family(index).map(Kinds::KindOf), None)
        }
        "SetName" | "SetTaggedValue" => element(|kind| kind.is_a(AbstractClass::ModelElement)),
        "SetGid" => element(|kind| kind.is_a(AbstractClass::SACMElement)),
        "SetFlag" if flag == Flag::IsCounter => (relationship, None, None),
        "SetFlag" => element(|kind| kind.is_a(AbstractClass::SACMElement)),
        "SetAssertionDeclaration" => element(|kind| kind.is_a(AbstractClass::Assertion)),
        "SetContent" => element(|kind| kind.is_a(AbstractClass::UtilityElement)),
        "SetKey" => element(|kind| kind == ElementKind::TaggedValue),
        "SetVersion" => element(|kind| kind == ElementKind::Artifact),
        "SetDate" => element(|kind| matches!(kind, ElementKind::Artifact | ElementKind::Event)),
        "SetLocation" => element(|kind| kind == ElementKind::Resource),
        "SetStartTime" | "SetEndTime" => element(|kind| kind == ElementKind::Activity),
        "SetValue" => element(|kind| matches!(kind, ElementKind::Term | ElementKind::Expression)),
        "SetExternalReference" => element(|kind| kind == ElementKind::Term),
        "SetReferences" | "AddReference" | "RemoveReference" => match end {
            ReferenceEnd::Source | ReferenceEnd::Target => (relationship, asset, asset),
            ReferenceEnd::Reasoning => {
                let reasoning = Some(Kinds::Exactly(ElementKind::ArgumentReasoning));
                (relationship, reasoning, reasoning)
            }
            _ => (None, None, None),
        },
        _ => (parent, None, None),
    }
}

/// A `RemoveReference` of a reference `doc` holds: the `n`-th of every id
/// (or, for `to_href`, every href) that an end of an element holds above
/// the end's lower bound, or `None` when there is none.
fn held_reference(doc: &Document, n: u16, to_href: bool) -> Option<Operation> {
    let mut held: Vec<Operation> = Vec::new();
    for element in doc.elements() {
        for &end in ReferenceEnd::ALL {
            // Only an end that holds more than its lower bound: removing from
            // a required end, such as a relationship's target, is refused.
            let count = element.references(end).len() + element.hrefs(end).len();
            if end
                .bounds(element.kind())
                .is_none_or(|(lower, _)| count <= lower)
            {
                continue;
            }
            if to_href {
                for href in element.hrefs(end) {
                    held.push(RemoveReference::new_href(element.id(), end, href.clone()).into());
                }
            } else {
                for id in element.references(end) {
                    held.push(RemoveReference::new(element.id(), end, id).into());
                }
            }
        }
    }
    let at = usize::from(n).checked_rem(held.len())?;
    held.into_iter().nth(at)
}

fn href() -> Href {
    Href::new(HREF).expect("LIB-RT-004: HREF is a valid href")
}

/// The operation `step` decodes to against `doc`, each by its primary
/// constructor, or the refusal of building a `CopyElements`.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per operation, in declaration order"
)]
fn operation(doc: &Document, step: &Step) -> Result<Operation, EditError> {
    let elements: Vec<(ElementId, ElementKind)> = doc
        .elements()
        .map(|element| (element.id().clone(), element.kind()))
        .collect();
    let ids: Vec<ElementId> = elements.iter().map(|(id, _)| id.clone()).collect();
    let index = usize::from(step.op) % OPERATIONS.len();
    let end = ReferenceEnd::ALL[usize::from(step.choice) % ReferenceEnd::ALL.len()];
    let flag = Flag::ALL[usize::from(step.choice) % Flag::ALL.len()];
    let (element_kinds, source_kinds, target_kinds) = if is_biased(step) {
        biased_kinds(index, end, flag)
    } else {
        (None, None, None)
    };
    let draw = |n: u16, kinds: Option<Kinds>| match kinds {
        Some(admits) => among(&elements, n, admits).unwrap_or_else(|| pick(&ids, n)),
        None => pick(&ids, n),
    };
    let target = draw(step.a, element_kinds);
    let other = draw(step.b, target_kinds);
    let list: Vec<ElementId> = match source_kinds {
        // A binding's participants: two or three of its family, consecutive
        // from the `b`-th, so distinct when the document holds that many.
        Some(family @ Kinds::KindOf(_)) => (0..2 + u16::from(step.picks.len() > 2))
            .filter_map(|i| among(&elements, step.b.wrapping_add(i), family))
            .collect(),
        _ if step.picks.is_empty() => vec![other.clone()],
        _ => step.picks.iter().map(|&n| draw(n, source_kinds)).collect(),
    };
    let text = step.text.as_str();
    let name = match step.choice % 3 {
        0 => LangString::new(text),
        1 => LangString::new(text).with_lang("en"),
        _ => LangString::new(text).with_lang("sv-SE"),
    };
    let even = step.choice % 2 == 0;
    let to_href = step.choice % 5 == 0;
    let caller = step.id.map(|n| ElementId::from(format!("x{n}")));
    macro_rules! create {
        ($create:expr) => {
            match &caller {
                Some(id) => Operation::from($create.with_id(id)),
                None => Operation::from($create),
            }
        };
    }
    macro_rules! package {
        ($create:ident) => {
            if even {
                create!($create::root(name))
            } else {
                create!($create::new(&target, name))
            }
        };
    }
    macro_rules! interface {
        ($create:ident) => {
            match (even, to_href) {
                (true, false) => create!($create::root(name, &other)),
                (true, true) => create!($create::root_href(name, href())),
                (false, false) => create!($create::new(&target, name, &other)),
                (false, true) => create!($create::new_href(&target, name, href())),
            }
        };
    }
    macro_rules! binding {
        ($create:ident) => {
            if even {
                create!($create::root(name, &list))
            } else {
                create!($create::new(&target, name, &list))
            }
        };
    }
    macro_rules! child {
        ($create:ident) => {
            create!($create::new(&target, name))
        };
    }
    macro_rules! relationship {
        ($create:ident) => {
            create!($create::new(&target, name, &list, &other))
        };
    }
    macro_rules! value {
        ($set:ident) => {
            if even {
                Operation::from($set::new(&target, text))
            } else {
                Operation::from($set::remove(&target))
            }
        };
    }
    let operation = match index {
        0 => package!(CreateAssuranceCasePackage),
        1 => interface!(CreateAssuranceCasePackageInterface),
        2 => binding!(CreateAssuranceCasePackageBinding),
        3 => package!(CreateArgumentPackage),
        4 => interface!(CreateArgumentPackageInterface),
        5 => binding!(CreateArgumentPackageBinding),
        6 => child!(CreateClaim),
        7 => child!(CreateArgumentReasoning),
        8 => relationship!(CreateAssertedInference),
        9 => relationship!(CreateAssertedContext),
        10 => child!(CreateArtifactReference),
        11 => child!(CreateArgumentGroup),
        12 => relationship!(CreateAssertedEvidence),
        13 => relationship!(CreateAssertedArtifactSupport),
        14 => relationship!(CreateAssertedArtifactContext),
        15 => child!(CreateDescription),
        16 => child!(CreateImplementationConstraint),
        17 => child!(CreateNote),
        18 => create!(CreateTaggedValue::new(&target, name.clone(), name)),
        19 => package!(CreateArtifactPackage),
        20 => interface!(CreateArtifactPackageInterface),
        21 => binding!(CreateArtifactPackageBinding),
        22 => child!(CreateProperty),
        23 => child!(CreateTechnique),
        24 => child!(CreateParticipant),
        25 => child!(CreateArtifactGroup),
        26 => child!(CreateArtifact),
        27 => child!(CreateEvent),
        28 => child!(CreateResource),
        29 => child!(CreateActivity),
        30 => create!(CreateArtifactAssetRelationship::new(
            &target,
            name,
            &list,
            [&other]
        )),
        31 => package!(CreateTerminologyPackage),
        32 => interface!(CreateTerminologyPackageInterface),
        33 => binding!(CreateTerminologyPackageBinding),
        34 => child!(CreateCategory),
        35 => create!(CreateTerm::new(&target, name, text)),
        36 => create!(CreateExpression::new(&target, name, text)),
        37 => child!(CreateTerminologyGroup),
        38 => SetName::new(&target, name).into(),
        39 => value!(SetGid),
        40 => SetFlag::new(&target, flag, step.choice % 2 == 1).into(),
        41 => {
            let all = AssertionDeclaration::ALL;
            SetAssertionDeclaration::new(&target, all[usize::from(step.choice) % all.len()]).into()
        }
        42 => {
            if even {
                SetContent::new(&target, name).into()
            } else {
                SetContent::remove(&target).into()
            }
        }
        43 => SetKey::new(&target, name).into(),
        44 => SetTaggedValue::new(&target, text, name).into(),
        45 => value!(SetVersion),
        46 => value!(SetDate),
        47 => {
            if even {
                SetLocation::new(&target, name).into()
            } else {
                SetLocation::remove(&target).into()
            }
        }
        48 => value!(SetStartTime),
        49 => value!(SetEndTime),
        50 => SetValue::new(&target, text).into(),
        51 => value!(SetExternalReference),
        52 => SetReferences::new(&target, end, &list).into(),
        53 => {
            if to_href {
                AddReference::new_href(&target, end, href()).into()
            } else {
                AddReference::new(&target, end, &other).into()
            }
        }
        54 => match is_biased(step)
            .then(|| held_reference(doc, step.a, to_href))
            .flatten()
        {
            Some(held) => held,
            None if to_href => RemoveReference::new_href(&target, end, href()).into(),
            None => RemoveReference::new(&target, end, &other).into(),
        },
        55 => {
            if even {
                ReorderElements::roots(&list).into()
            } else {
                ReorderElements::new(&target, &list).into()
            }
        }
        56 => {
            if even {
                MoveElement::root(&target).into()
            } else {
                MoveElement::new(&target, &other).into()
            }
        }
        57 => {
            if even {
                CopyElements::root(doc, &list)?.into()
            } else {
                CopyElements::new(doc, &list, &target)?.into()
            }
        }
        _ => {
            let reference = match step.choice % 3 {
                0 => ReferenceDeletePolicy::RejectIfReferenced,
                1 => ReferenceDeletePolicy::DeleteReferencingRelationships,
                _ => ReferenceDeletePolicy::ScrubReferences,
            };
            let package = if (step.choice / 3) % 2 == 0 {
                PackageDeletePolicy::RejectIfNonEmpty
            } else {
                PackageDeletePolicy::DeleteRecursively
            };
            let cross = if (step.choice / 6) % 2 == 0 {
                CrossPackageReferencePolicy::RejectIfExternalReferencesExist
            } else {
                CrossPackageReferencePolicy::ApplyReferencePolicy
            };
            DeleteElement::new(&target)
                .with_reference_policy(reference)
                .with_package_policy(package)
                .with_cross_package_policy(cross)
                .into()
        }
    };
    Ok(operation)
}

/// The decoder selects each [`Operation`] variant once, in declaration
/// order, and [`OPERATIONS`] lists exactly the variants `src/edit.rs`
/// declares: a new variant fails here until the decoder builds it.
fn assert_the_decoder_covers_every_operation() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/edit.rs"))
        .unwrap_or_else(|error| panic!("{RT_004}: setup: cannot read src/edit.rs: {error}"));
    let declared: Vec<&str> = source
        .lines()
        .skip_while(|line| *line != "pub enum Operation {")
        .skip(1)
        .take_while(|line| *line != "}")
        .filter_map(|line| {
            let variant = line.strip_prefix("    ")?;
            let (name, _) = variant.split_once('(')?;
            name.starts_with(|c: char| c.is_ascii_uppercase())
                .then_some(name)
        })
        .collect();
    assert_eq!(
        declared, OPERATIONS,
        "{RT_004}: setup: OPERATIONS is not the declared list of Operation variants"
    );
    let start = start();
    for (index, expected) in OPERATIONS.iter().enumerate() {
        let step = Step {
            op: u8::try_from(index).expect("LIB-RT-004: fewer than 256 operations"),
            a: 0,
            b: 1,
            picks: vec![1],
            text: "T".to_owned(),
            choice: 1,
            id: None,
        };
        let decoded = operation(&start, &step).unwrap_or_else(|error| {
            panic!("{RT_004}: setup: operation {index} does not build: {error}")
        });
        assert_eq!(
            decoded.operation_name(),
            *expected,
            "{RT_004}: setup: op {index} decodes to the wrong operation"
        );
        assert_eq!(
            created_kind(index).is_some(),
            expected.starts_with("Create"),
            "{RT_004}: setup: {expected} names no element kind for its biased parent"
        );
    }
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{RT_004}: setup: {operation:?} was refused: {error}"),
    }
}

fn only_created(mutation: &Mutation) -> ElementId {
    match mutation.created_ids() {
        [only] => only.clone(),
        other => panic!("{RT_004}: setup: {mutation:?} created {other:?}, not one element"),
    }
}

static START: LazyLock<Document> = LazyLock::new(|| {
    let mut doc = Document::new();
    applied(
        &mut doc,
        CreateAssuranceCasePackage::root("Case").with_id("acp"),
    );
    applied(
        &mut doc,
        CreateArgumentPackage::new("acp", "P").with_id("p"),
    );
    applied(
        &mut doc,
        CreateArtifactPackage::new("acp", "A").with_id("a"),
    );
    applied(
        &mut doc,
        CreateTerminologyPackage::new("acp", "T").with_id("t"),
    );
    applied(&mut doc, CreateTerm::new("t", "Term", "v").with_id("term"));
    doc
});

/// The `AssuranceCasePackage` `acp` holding the `ArgumentPackage` `p`, the
/// `ArtifactPackage` `a` and the `TerminologyPackage` `t`, which holds the
/// `Term` `term`: the only kind whose sets (`value`, `externalReference`)
/// a sequence rarely reaches when it must create the element first.
fn start() -> Document {
    START.clone()
}

/// One create of each kind the edit API creates, with generated ids, under
/// fresh roots, applied to a clone of `doc`: the ids show the next generated
/// id of every kind. Copied from `batch.rs`, with the terminology kinds and
/// the package interfaces and bindings added.
fn next_generated_ids(doc: &Document) -> Vec<ElementId> {
    let mut probe = doc.clone();
    let name = || LangString::new("probe").with_lang("en");
    let case = only_created(&applied(
        &mut probe,
        CreateAssuranceCasePackage::root(name()),
    ));
    let arguments = only_created(&applied(&mut probe, CreateArgumentPackage::root(name())));
    let claim = only_created(&applied(&mut probe, CreateClaim::new(&arguments, name())));
    let (p, c) = (&arguments, &claim);
    let rest: Vec<Operation> = vec![
        CreateArgumentReasoning::new(p, name()).into(),
        CreateArtifactReference::new(p, name()).into(),
        CreateArgumentGroup::new(p, name()).into(),
        CreateAssertedInference::new(p, name(), [c], c).into(),
        CreateAssertedContext::new(p, name(), [c], c).into(),
        CreateAssertedEvidence::new(p, name(), [c], c).into(),
        CreateAssertedArtifactSupport::new(p, name(), [c], c).into(),
        CreateAssertedArtifactContext::new(p, name(), [c], c).into(),
        CreateDescription::new(c, name()).into(),
        CreateImplementationConstraint::new(c, name()).into(),
        CreateNote::new(c, name()).into(),
        CreateTaggedValue::new(c, name(), name()).into(),
    ];
    let mut ids = vec![case.clone(), arguments.clone(), claim.clone()];
    for operation in rest {
        ids.push(only_created(&applied(&mut probe, operation)));
    }
    let artifacts = only_created(&applied(&mut probe, CreateArtifactPackage::root(name())));
    let a = &artifacts;
    let artifact_rest: Vec<Operation> = vec![
        CreateProperty::new(a, name()).into(),
        CreateTechnique::new(a, name()).into(),
        CreateParticipant::new(a, name()).into(),
        CreateArtifactGroup::new(a, name()).into(),
        CreateArtifact::new(a, name()).into(),
        CreateEvent::new(a, name()).into(),
        CreateResource::new(a, name()).into(),
        CreateActivity::new(a, name()).into(),
    ];
    ids.push(artifacts.clone());
    for operation in artifact_rest {
        ids.push(only_created(&applied(&mut probe, operation)));
    }
    let terms = only_created(&applied(&mut probe, CreateTerminologyPackage::root(name())));
    let t = &terms;
    let terminology_rest: Vec<Operation> = vec![
        CreateCategory::new(t, name()).into(),
        CreateTerm::new(t, name(), "v").into(),
        CreateExpression::new(t, name(), "v").into(),
        CreateTerminologyGroup::new(t, name()).into(),
    ];
    ids.push(terms.clone());
    for operation in terminology_rest {
        ids.push(only_created(&applied(&mut probe, operation)));
    }
    let interfaces: Vec<Operation> = vec![
        CreateAssuranceCasePackageInterface::root(name(), &case).into(),
        CreateArgumentPackageInterface::root(name(), &arguments).into(),
        CreateArtifactPackageInterface::root(name(), &artifacts).into(),
        CreateTerminologyPackageInterface::root(name(), &terms).into(),
    ];
    let mut declared = Vec::new();
    for operation in interfaces {
        let id = only_created(&applied(&mut probe, operation));
        declared.push(id.clone());
        ids.push(id);
    }
    let bindings: Vec<Operation> = vec![
        CreateAssuranceCasePackageBinding::root(name(), [&case, &declared[0]]).into(),
        CreateArgumentPackageBinding::root(name(), [&arguments, &declared[1]]).into(),
        CreateArtifactPackageBinding::root(name(), [&artifacts, &declared[2]]).into(),
        CreateTerminologyPackageBinding::root(name(), [&terms, &declared[3]]).into(),
    ];
    for operation in bindings {
        ids.push(only_created(&applied(&mut probe, operation)));
    }
    ids
}

/// How often each operation of [`OPERATIONS`] was tried and accepted over a
/// property's run, shrinking included.
struct Tally {
    tried: [AtomicUsize; OPERATIONS.len()],
    accepted: [AtomicUsize; OPERATIONS.len()],
}

impl Tally {
    fn new() -> Self {
        Self {
            tried: [const { AtomicUsize::new(0) }; OPERATIONS.len()],
            accepted: [const { AtomicUsize::new(0) }; OPERATIONS.len()],
        }
    }

    fn record(&self, name: &str, accepted: bool) {
        if let Some(index) = OPERATIONS.iter().position(|known| *known == name) {
            self.tried[index].fetch_add(1, Ordering::Relaxed);
            if accepted {
                self.accepted[index].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// The run accepted every operation at least once: the generator reaches
    /// each. Not checked when `PROPTEST_CASES` sets fewer cases.
    fn assert_every_operation_accepted(&self, property: &str) {
        if std::env::var_os("PROPTEST_CASES").is_some() {
            return;
        }
        let never: Vec<String> = OPERATIONS
            .iter()
            .zip(self.tried.iter().zip(&self.accepted))
            .filter(|(_, (_, accepted))| accepted.load(Ordering::Relaxed) == 0)
            .map(|(name, (tried, _))| format!("{name} (0 of {})", tried.load(Ordering::Relaxed)))
            .collect();
        assert!(
            never.is_empty(),
            "{RT_004}: setup: {property} never accepts {}",
            never.join(", ")
        );
    }
}

/// Applies `steps` to `doc` one by one, each decoded against the document so
/// far, ignoring refusals, and records each in `tally`.
fn apply_steps(doc: &mut Document, steps: &[Step], tally: &Tally) {
    for step in steps {
        if let Ok(operation) = operation(doc, step) {
            let name = operation.operation_name();
            let accepted = edit::apply(doc, operation).is_ok();
            tally.record(name, accepted);
        }
    }
}

/// The first differences between `left` and `right`, for a message.
fn first_differences(left: &Document, right: &Document) -> String {
    let differences = semantic_compare(left, right);
    format!(
        "{} difference(s), first: {:?}",
        differences.len(),
        differences.iter().take(3).collect::<Vec<_>>()
    )
}

fn read_options(mode: Mode) -> ReadOptions {
    ReadOptions::default().with_mode(mode)
}

fn write_options(mode: Mode) -> WriteOptions {
    WriteOptions::default().with_mode(mode)
}

/// `doc` writes in `mode`, the same bytes twice; reads back in `mode` with no
/// diagnostic and `==`; and writes the same bytes again.
fn assert_writes_and_reads_back(doc: &Document, mode: Mode) -> Result<(), TestCaseError> {
    let written = xmi::write(doc, &write_options(mode));
    prop_assert!(
        written.is_ok(),
        "{RT_004}: a {mode} write is refused: {:?}",
        written
    );
    let first = written.unwrap_or_default();
    let second = xmi::write(doc, &write_options(mode));
    prop_assert!(
        second.as_ref() == Ok(&first),
        "{RT_004}: a second {mode} write gives other bytes"
    );
    let read = xmi::read(first.as_bytes(), &read_options(mode));
    let output = match read {
        Ok(output) => output,
        Err(error) => {
            return Err(TestCaseError::fail(format!(
                "{RT_004}: the {mode} output does not read back in {mode} mode: {error}\n{first}"
            )));
        }
    };
    prop_assert!(
        output.diagnostics().is_empty(),
        "{RT_004}: the {mode} read back has diagnostics: {:?}\n{first}",
        output.diagnostics()
    );
    prop_assert!(
        output.document() == doc,
        "{RT_004}: the {mode} read back is not == to the document: {}\n{first}",
        first_differences(doc, output.document())
    );
    let third = xmi::write(output.document(), &write_options(mode));
    prop_assert!(
        third.as_ref() == Ok(&first),
        "{RT_004}: writing the {mode} read back gives other bytes"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The properties
// ---------------------------------------------------------------------------

/// Property 1 (D-RT): a built document writes and reads back `==` in both
/// modes, and the same document writes the same bytes.
#[test]
fn lib_rt_004_a_built_document_writes_and_reads_back_equal_in_both_modes() {
    assert_the_decoder_covers_every_operation();
    let tally = Tally::new();
    run(256, 40_401, &steps(), |steps| {
        let mut doc = start();
        apply_steps(&mut doc, &steps, &tally);
        if doc.roots().next().is_none() {
            for mode in MODES {
                prop_assert_eq!(
                    xmi::write(&doc, &write_options(mode)),
                    Err(WriteError::NoRoot),
                    "{}: a {} write of a document with no root",
                    RT_004,
                    mode
                );
            }
            return Ok(());
        }
        for mode in MODES {
            assert_writes_and_reads_back(&doc, mode)?;
        }
        Ok(())
    });
    tally.assert_every_operation_accepted("property 1");
}

/// Property 2 (D-ATOMIC): a refused edit leaves the document `==`, with the
/// same revision and next generated ids, and `edit::check` gives the same
/// error. Over the run, some refusals are LIB-EDIT-025's.
#[test]
fn lib_rt_004_a_refused_edit_changes_nothing() {
    let unrepresentable = AtomicUsize::new(0);
    run(64, 40_402, &steps(), |steps| {
        let mut doc = start();
        for step in &steps {
            let Ok(operation) = operation(&doc, step) else {
                continue;
            };
            let before = doc.clone();
            let revision = doc.revision();
            let Err(error) = edit::apply(&mut doc, operation.clone()) else {
                continue;
            };
            if matches!(error, EditError::UnrepresentableCharacter { .. }) {
                unrepresentable.fetch_add(1, Ordering::Relaxed);
            }
            prop_assert!(
                doc == before,
                "{RT_004}: refused {operation:?} ({error}) changed the document: {}",
                first_differences(&before, &doc)
            );
            prop_assert_eq!(
                doc.revision(),
                revision,
                "{}: refused {:?} changed the revision",
                RT_004,
                operation
            );
            prop_assert_eq!(
                next_generated_ids(&doc),
                next_generated_ids(&before),
                "{}: refused {:?} changed the next generated ids",
                RT_004,
                operation
            );
            prop_assert_eq!(
                edit::check(&before, &operation),
                Err(error),
                "{}: edit::check of {:?} gives another answer",
                RT_004,
                operation
            );
        }
        Ok(())
    });
    if std::env::var_os("PROPTEST_CASES").is_none() {
        assert!(
            unrepresentable.load(Ordering::Relaxed) > 0,
            "{RT_004}: setup: no edit was refused for a character XML 1.0 cannot represent"
        );
    }
}

/// The most operations in one list of property 3.
const LIST_LENGTH: usize = 6;

/// Property 3's draw: steps, each with whether to keep its operation when
/// the document refuses it (one in twenty).
fn list_steps() -> impl Strategy<Value = Vec<(Step, bool)>> {
    vec((step(), proptest::bool::weighted(0.05)), 0..=24)
}

/// Up to [`LIST_LENGTH`] operations from `drawn`, each decoded against the
/// document as the operations before it leave it. An operation that document
/// refuses is kept only when its step says so, so most lists commit and the
/// rest are refused at a known operation.
fn operation_list(start: &Document, drawn: &[(Step, bool)]) -> Vec<Operation> {
    let mut doc = start.clone();
    let mut operations = Vec::new();
    for (step, keep_refused) in drawn {
        if operations.len() == LIST_LENGTH {
            break;
        }
        let Ok(operation) = operation(&doc, step) else {
            continue;
        };
        let accepted = edit::check(&doc, &operation).is_ok();
        if accepted {
            let _ = edit::apply(&mut doc, operation.clone());
        }
        if accepted || *keep_refused {
            operations.push(operation);
        }
    }
    operations
}

/// Property 3 (ADR 0032): `apply_all` over a list of operations equals
/// applying them one by one when it commits; when it refuses, the document
/// is as before and the error names the first refusal.
#[test]
fn lib_rt_004_apply_all_is_all_or_nothing() {
    run(128, 40_403, &list_steps(), |drawn| {
        let start = start();
        let operations = operation_list(&start, &drawn);
        let mut one_by_one = start.clone();
        let mut first_refusal = None;
        for (index, operation) in operations.iter().enumerate() {
            if let Err(error) = edit::apply(&mut one_by_one, operation.clone()) {
                first_refusal = Some((index, error));
                break;
            }
        }
        let mut doc = start.clone();
        match edit::apply_all(&mut doc, operations.clone()) {
            Ok(_) => {
                prop_assert!(
                    first_refusal.is_none(),
                    "{RT_004}: apply_all committed what one by one refuses: {first_refusal:?}"
                );
                prop_assert!(
                    doc == one_by_one,
                    "{RT_004}: apply_all is not == to one by one: {}",
                    first_differences(&one_by_one, &doc)
                );
            }
            Err(error) => {
                prop_assert!(
                    doc == start,
                    "{RT_004}: a refused apply_all changed the document: {}",
                    first_differences(&start, &doc)
                );
                prop_assert_eq!(
                    doc.revision(),
                    start.revision(),
                    "{}: a refused apply_all changed the revision",
                    RT_004
                );
                prop_assert_eq!(
                    next_generated_ids(&doc),
                    next_generated_ids(&start),
                    "{}: a refused apply_all changed the next generated ids",
                    RT_004
                );
                let Some((index, refusal)) = first_refusal else {
                    return Err(TestCaseError::fail(format!(
                        "{RT_004}: apply_all refused ({error}) what one by one accepts"
                    )));
                };
                prop_assert_eq!(
                    error.index(),
                    index,
                    "{}: apply_all names another operation",
                    RT_004
                );
                prop_assert_eq!(
                    error.error(),
                    &refusal,
                    "{}: apply_all gives another refusal",
                    RT_004
                );
            }
        }
        Ok(())
    });
}

/// Property 4 (ADR 0032): a batch whose closure returns `Err` leaves the
/// document `==`, with the same revision and next generated ids.
#[test]
fn lib_rt_004_a_batch_that_returns_an_error_changes_nothing() {
    run(128, 40_404, &steps(), |steps| {
        let mut doc = start();
        let before = doc.clone();
        let outcome = edit::batch(&mut doc, |tx| {
            for step in &steps {
                if let Ok(operation) = operation(tx.document(), step) {
                    let _ = tx.apply(operation);
                }
            }
            Err("rolled back")
        });
        prop_assert!(
            matches!(outcome, Err("rolled back")),
            "{RT_004}: the batch does not return the closure's error: {outcome:?}"
        );
        prop_assert!(
            doc == before,
            "{RT_004}: a rolled-back batch changed the document: {}",
            first_differences(&before, &doc)
        );
        prop_assert_eq!(
            doc.revision(),
            before.revision(),
            "{}: a rolled-back batch changed the revision",
            RT_004
        );
        prop_assert_eq!(
            next_generated_ids(&doc),
            next_generated_ids(&before),
            "{}: a rolled-back batch changed the next generated ids",
            RT_004
        );
        Ok(())
    });
}

/// Property 5 (ADR 0032, LIB-RT-005): a committed batch's net ids are those
/// of `changes(before, after)`; created ids compare as sets, since a batch
/// lists them in creation order and `changes` in pre-order.
#[test]
fn lib_rt_004_batch_net_ids_equal_compare_changes() {
    run(128, 40_405, &steps(), |steps| {
        let mut doc = start();
        let before = doc.clone();
        let outcome = edit::batch(&mut doc, |tx| {
            for step in &steps {
                if let Ok(operation) = operation(tx.document(), step) {
                    let _ = tx.apply(operation);
                }
            }
            Ok::<(), Infallible>(())
        });
        let batch = match outcome {
            Ok(batch) => batch,
            Err(never) => match never {},
        };
        let found = changes(&before, &doc);
        prop_assert_eq!(
            batch.changed_ids(),
            found.changed_ids(),
            "{}: changed ids",
            RT_004
        );
        prop_assert_eq!(
            batch.deleted_ids(),
            found.deleted_ids(),
            "{}: deleted ids",
            RT_004
        );
        let mut created = batch.created_ids().to_vec();
        created.sort();
        let mut expected = found.created_ids().to_vec();
        expected.sort();
        prop_assert_eq!(created, expected, "{}: created ids", RT_004);
        let lists_empty = found.created_ids().is_empty()
            && found.changed_ids().is_empty()
            && found.deleted_ids().is_empty();
        prop_assert_eq!(
            batch.is_empty(),
            lists_empty,
            "{}: is_empty() disagrees with the lists of changes",
            RT_004
        );
        Ok(())
    });
}

/// The documents of [`STRICT_FIXTURES`], each read strictly with no
/// diagnostic and written strictly.
static STRICT_DOCUMENTS: LazyLock<Vec<Document>> = LazyLock::new(|| {
    STRICT_FIXTURES
        .iter()
        .map(|fixture| {
            let path = format!("{}/tests/data/sacm23/{fixture}", env!("CARGO_MANIFEST_DIR"));
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("{RT_004}: setup: cannot read {path}: {error}"));
            let output = xmi::read(&bytes, &read_options(Mode::Strict)).unwrap_or_else(|error| {
                panic!("{RT_004}: setup: {fixture} does not read: {error}")
            });
            assert!(
                output.diagnostics().is_empty(),
                "{RT_004}: setup: {fixture} reads with diagnostics: {:?}",
                output.diagnostics()
            );
            if let Err(error) = xmi::write(output.document(), &write_options(Mode::Strict)) {
                panic!("{RT_004}: setup: {fixture} does not write strictly: {error}");
            }
            output.into_document()
        })
        .collect()
});

/// Property 6 (D-RT, the `xmi::write` guarantee): a document from a strict
/// read that a strict write accepts, after any edits `edit::apply` accepted,
/// writes strictly and reads back `==`; once no root is left, the write
/// refuses with `NoRoot` (LIB-EDIT-026).
#[test]
fn lib_rt_004_a_strict_read_document_after_accepted_edits_writes_strictly_and_reads_back_equal() {
    let documents = &*STRICT_DOCUMENTS;
    let tally = Tally::new();
    run(
        64,
        40_406,
        &(0..documents.len(), steps()),
        |(index, steps)| {
            let mut doc = documents[index].clone();
            apply_steps(&mut doc, &steps, &tally);
            let written = xmi::write(&doc, &write_options(Mode::Strict));
            // A delete of the last root is accepted; the write then refuses
            // with `NoRoot`, as LIB-EDIT-026 states.
            if doc.roots().next().is_none() {
                prop_assert_eq!(
                    written,
                    Err(WriteError::NoRoot),
                    "{}: {} with no root left",
                    RT_004,
                    STRICT_FIXTURES[index]
                );
                return Ok(());
            }
            let text = match written {
                Ok(text) => text,
                Err(error) => {
                    return Err(TestCaseError::fail(format!(
                        "{RT_004}: {} after accepted edits does not write strictly: {error}",
                        STRICT_FIXTURES[index]
                    )));
                }
            };
            let output = match xmi::read(text.as_bytes(), &read_options(Mode::Strict)) {
                Ok(output) => output,
                Err(error) => {
                    return Err(TestCaseError::fail(format!(
                        "{RT_004}: {} after accepted edits does not read back: {error}\n{text}",
                        STRICT_FIXTURES[index]
                    )));
                }
            };
            prop_assert!(
                output.diagnostics().is_empty(),
                "{RT_004}: {} read back has diagnostics: {:?}",
                STRICT_FIXTURES[index],
                output.diagnostics()
            );
            prop_assert!(
                output.document() == &doc,
                "{RT_004}: {} read back is not ==: {}",
                STRICT_FIXTURES[index],
                first_differences(&doc, output.document())
            );
            Ok(())
        },
    );
    tally.assert_every_operation_accepted("property 6");
}
