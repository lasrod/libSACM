//! Tests for element kind metadata: `ElementKind`, `AbstractClass` and
//! `MetamodelPackage` (LIB-API-003, ADR 0010).
//!
//! Most of these are drift tests. They compare the hand-written enums with
//! `docs/sacm/metamodel-inventory.md`, which is derived from the normative
//! machine-readable model ptc/22-03-13, so a change on either side fails here.
//!
//! The `#[non_exhaustive]` part of acceptance item 8 is not checked here: only a
//! compile-fail test (an exhaustive `match` from outside the crate) could show
//! it, and that needs a harness such as `trybuild`. The verifier checks it in
//! the source and the rustdoc.

use std::collections::BTreeSet;
use std::fmt::{Debug, Display};
use std::hash::Hash;

use sacm::model::{AbstractClass, ElementKind, MetamodelPackage};

const INVENTORY: &str = include_str!("../../../docs/sacm/metamodel-inventory.md");

/// Inventory classes that are values, not `SACMElement`s, and so have no kind.
const VALUE_TYPES: [&str; 3] = ["LangString", "MultiLangString", "ExpressionLangString"];

/// The five packages of the SACM 2.3 metamodel.
const PACKAGES: [&str; 5] = [
    "Base",
    "AssuranceCase",
    "Terminology",
    "Argumentation",
    "Artifact",
];

/// One `### Class` section of the inventory.
struct Class {
    name: &'static str,
    package: &'static str,
    is_abstract: bool,
    /// The superclasses, nearest first, from the "Generalization chain" line.
    chain: Vec<&'static str>,
}

/// Parses the class sections under the inventory's `## Package` headings:
///
/// ```text
/// ## Package `Argumentation`
/// ### AssertedRelationship (abstract)
/// Generalization chain: Assertion -> ArgumentAsset -> ... -> Element
/// ```
fn inventory() -> Vec<Class> {
    let mut classes: Vec<Class> = Vec::new();
    let mut package = None;
    for line in INVENTORY.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            // Any other `##` section (enumerations, tables) ends the packages.
            package = heading
                .strip_prefix("Package `")
                .and_then(|rest| rest.strip_suffix('`'));
        } else if let (Some(package), Some(heading)) = (package, line.strip_prefix("### ")) {
            let (name, is_abstract) = match heading.strip_suffix(" (abstract)") {
                Some(name) => (name, true),
                None => (heading, false),
            };
            classes.push(Class {
                name,
                package,
                is_abstract,
                chain: Vec::new(),
            });
        } else if let (Some(_), Some(chain)) =
            (package, line.strip_prefix("Generalization chain: "))
        {
            if let Some(class) = classes.last_mut() {
                class.chain = chain.split(" -> ").collect();
            }
        }
    }
    classes
}

fn find<'i>(inventory: &'i [Class], name: &str) -> &'i Class {
    let Some(class) = inventory.iter().find(|class| class.name == name) else {
        panic!("LIB-API-003: {name} is not a class of the inventory");
    };
    class
}

fn kinds() -> impl Iterator<Item = ElementKind> {
    ElementKind::ALL.iter().copied()
}

fn abstract_classes() -> impl Iterator<Item = AbstractClass> {
    AbstractClass::ALL.iter().copied()
}

#[test]
fn lib_api_003_inventory_parses_into_the_five_packages() {
    // Guards the parser: if the inventory's format changes, the drift tests
    // below must fail rather than compare against nothing.
    let inventory = inventory();
    let packages: BTreeSet<&str> = inventory.iter().map(|class| class.package).collect();
    assert_eq!(
        packages,
        BTreeSet::from(PACKAGES),
        "LIB-API-003: inventory packages"
    );
    for class in &inventory {
        let rooted = class.chain.last() == Some(&"Element");
        assert!(
            rooted || class.name == "Element",
            "LIB-API-003: {} has no generalization chain ending in Element",
            class.name
        );
        for superclass in &class.chain {
            find(&inventory, superclass);
        }
    }
}

// --- 1. ElementKind::ALL --------------------------------------------------

#[test]
fn lib_api_003_all_holds_each_concrete_inventory_class_once() {
    let expected: BTreeSet<&str> = inventory()
        .iter()
        .filter(|class| !class.is_abstract && !VALUE_TYPES.contains(&class.name))
        .map(|class| class.name)
        .collect();
    let names: Vec<&str> = kinds().map(ElementKind::name).collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();

    assert_eq!(
        names.len(),
        unique.len(),
        "LIB-API-003: ElementKind::ALL repeats a kind: {names:?}"
    );
    assert_eq!(
        unique, expected,
        "LIB-API-003: ElementKind::ALL differs from the inventory"
    );
    assert_eq!(
        names.len(),
        38,
        "LIB-API-003: SACM 2.3 has 38 concrete SACMElement classes"
    );
}

#[test]
fn lib_api_003_value_types_have_no_kind() {
    let inventory = inventory();
    for value_type in VALUE_TYPES {
        assert!(
            !find(&inventory, value_type).is_abstract,
            "LIB-API-003: {value_type} is expected to be a concrete inventory class"
        );
        assert!(
            kinds().all(|kind| kind.name() != value_type),
            "LIB-API-003: the value type {value_type} has an ElementKind"
        );
    }
}

// --- 2. name() and Display ------------------------------------------------

#[test]
fn lib_api_003_kind_display_is_its_sacm_name() {
    for kind in kinds() {
        assert_eq!(
            kind.to_string(),
            kind.name(),
            "LIB-API-003: Display of {kind:?}"
        );
    }
}

#[test]
fn lib_api_003_artifact_asset_relationship_has_the_text_spelling() {
    // D-TEXT-MODEL: clause 12.14 names it ArtifactAssetRelationship, a kind of
    // ArtifactAsset; the machine-readable model's ArtifactAssertedRelationship is
    // an import spelling only.
    let kind = ElementKind::ArtifactAssetRelationship;
    assert_eq!(
        kind.name(),
        "ArtifactAssetRelationship",
        "LIB-API-003: spelling"
    );
    assert!(
        kind.is_a(AbstractClass::ArtifactAsset),
        "LIB-API-003: ArtifactAssetRelationship is an ArtifactAsset (clause 12.14)"
    );
    assert!(
        kinds().all(|kind| kind.name() != "ArtifactAssertedRelationship"),
        "LIB-API-003: the machine-readable spelling is a kind name"
    );
}

// --- 3. package() ---------------------------------------------------------

#[test]
fn lib_api_003_kind_package_matches_the_inventory_section() {
    let inventory = inventory();
    for kind in kinds() {
        assert_eq!(
            kind.package().name(),
            find(&inventory, kind.name()).package,
            "LIB-API-003: package of {kind}"
        );
    }
}

#[test]
fn lib_api_003_package_names_are_the_five_sacm_packages() {
    let packages: Vec<MetamodelPackage> = kinds()
        .map(ElementKind::package)
        .chain(abstract_classes().map(AbstractClass::package))
        .collect();
    let names: BTreeSet<&str> = packages.iter().map(|package| package.name()).collect();
    assert_eq!(
        names,
        BTreeSet::from(PACKAGES),
        "LIB-API-003: package names"
    );
    for package in packages {
        assert_eq!(
            package.to_string(),
            package.name(),
            "LIB-API-003: Display of {package:?}"
        );
    }
}

// --- 4. AbstractClass -----------------------------------------------------

#[test]
fn lib_api_003_abstract_class_all_holds_each_abstract_inventory_class_once() {
    let expected: BTreeSet<&str> = inventory()
        .iter()
        .filter(|class| class.is_abstract)
        .map(|class| class.name)
        .collect();
    let names: Vec<&str> = abstract_classes().map(AbstractClass::name).collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();

    assert_eq!(
        names.len(),
        unique.len(),
        "LIB-API-003: AbstractClass::ALL repeats a class: {names:?}"
    );
    assert_eq!(
        unique, expected,
        "LIB-API-003: AbstractClass::ALL differs from the inventory"
    );
    assert_eq!(
        names.len(),
        13,
        "LIB-API-003: SACM 2.3 has 13 abstract classes"
    );
}

#[test]
fn lib_api_003_abstract_class_package_and_display_match_the_inventory() {
    let inventory = inventory();
    for class in abstract_classes() {
        assert_eq!(
            class.package().name(),
            find(&inventory, class.name()).package,
            "LIB-API-003: package of {class}"
        );
        assert_eq!(
            class.to_string(),
            class.name(),
            "LIB-API-003: Display of {class:?}"
        );
    }
}

// --- 5. is_a, all 38 × 13 pairs -------------------------------------------

#[test]
fn lib_api_003_is_a_follows_the_inventory_chains() {
    let inventory = inventory();
    let mut wrong = Vec::new();
    for kind in kinds() {
        let chain = &find(&inventory, kind.name()).chain;
        for class in abstract_classes() {
            let expected = chain.contains(&class.name());
            if kind.is_a(class) != expected {
                wrong.push(format!("{kind}.is_a({class}) should be {expected}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "LIB-API-003: is_a disagrees with the inventory:\n{}",
        wrong.join("\n")
    );
}

// --- 6. is_kind_of, all 38 × 38 pairs -------------------------------------

#[test]
fn lib_api_003_is_kind_of_follows_the_inventory_chains() {
    let inventory = inventory();
    let mut wrong = Vec::new();
    for kind in kinds() {
        let chain = &find(&inventory, kind.name()).chain;
        for other in kinds() {
            let expected = kind == other || chain.contains(&other.name());
            if kind.is_kind_of(other) != expected {
                wrong.push(format!("{kind}.is_kind_of({other}) should be {expected}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "LIB-API-003: is_kind_of disagrees with the inventory:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn lib_api_003_only_interfaces_and_bindings_are_kinds_of_another_kind() {
    let related: BTreeSet<(&str, &str)> = kinds()
        .flat_map(|kind| kinds().map(move |other| (kind, other)))
        .filter(|&(kind, other)| kind != other && kind.is_kind_of(other))
        .map(|(kind, other)| (kind.name(), other.name()))
        .collect();
    let expected = BTreeSet::from([
        ("AssuranceCasePackageInterface", "AssuranceCasePackage"),
        ("AssuranceCasePackageBinding", "AssuranceCasePackage"),
        ("TerminologyPackageInterface", "TerminologyPackage"),
        ("TerminologyPackageBinding", "TerminologyPackage"),
        ("ArgumentPackageInterface", "ArgumentPackage"),
        ("ArgumentPackageBinding", "ArgumentPackage"),
        ("ArtifactPackageInterface", "ArtifactPackage"),
        ("ArtifactPackageBinding", "ArtifactPackage"),
    ]);
    assert_eq!(
        related, expected,
        "LIB-API-003: kinds related by is_kind_of"
    );
}

// --- 7. is_package --------------------------------------------------------

#[test]
fn lib_api_003_is_package_holds_for_packages_interfaces_and_bindings() {
    for kind in kinds() {
        let name = kind.name();
        let expected = name.ends_with("Package")
            || name.ends_with("PackageInterface")
            || name.ends_with("PackageBinding");
        assert_eq!(
            kind.is_package(),
            expected,
            "LIB-API-003: is_package of {kind}"
        );
    }
    assert_eq!(
        kinds().filter(|kind| kind.is_package()).count(),
        12,
        "LIB-API-003: four packages, each with an interface and a binding"
    );
}

// --- 8. Plain value types -------------------------------------------------

fn assert_value_type<T: Copy + Eq + Ord + Hash + Debug + Display + Send + Sync>() {}

#[test]
fn lib_api_003_kind_types_are_plain_values() {
    assert_value_type::<ElementKind>();
    assert_value_type::<AbstractClass>();
    assert_value_type::<MetamodelPackage>();
}
