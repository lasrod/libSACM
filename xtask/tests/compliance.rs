//! The unit documents of the four model compliance points hold every class
//! and property of their packages (SACM23-CP-001 to SACM23-CP-004;
//! formal/23-05-08 §2.2 to §2.5; ADR 0043).
//!
//! formal/23-05-08 §2.2: software at the Argumentation Model compliance
//! point "shall be able to import and export XMI documents that conform with
//! the SACM XML Schema produced by applying XMI rules to the normative MOF
//! metamodel defined in the Argumentation subpackage of the SACM
//! specification, including the common elements defined in the Common and
//! Predefined diagrams of the SACM." §2.3 says the same of the Artifact
//! subpackage, and §2.4 and §2.5 of "this entire specification" (for §2.5,
//! spec defect D29; its unit is the TerminologyPackage).
//!
//! Each test walks one unit document,
//! `crates/sacm/tests/data/sacm23/cp/<unit>-unit.sacm.xmi`, against the
//! pinned ptc/22-03-13 with the recorded corrections (`xtask::metamodel`):
//!
//! - the root's class is its local name;
//! - for an object of class C, each attribute in no namespace must name a
//!   property of C, inherited ones included, and marks the class that
//!   declares it and its name seen; child elements in the XMI namespace are
//!   skipped; every other child must name a property of C and marks it seen;
//! - a child of a composition with neither `href` nor `xmi:idref` is an
//!   object, of the class `xmi:type` names (its local part), or else of the
//!   property's type; that class is marked seen and the walk goes into it;
//! - an unknown attribute, child or class fails the test.
//!
//! Then every concrete class of the point's packages must have been seen,
//! and every property a class of those packages declares. Coverage is per
//! declared property, not per (class, property) pair: per-class coverage is
//! LIB-TOOL-004's (`metamodel.rs`).
//!
//! The tests need the pinned model: see `common/spec.rs` for where it is
//! found and when a test is skipped.

#[path = "common/spec.rs"]
mod spec;

use std::collections::BTreeSet;
use std::fs;

use xtask::metamodel::{MODEL_FILE, Metamodel, PropertyKind, load};

use spec::spec_dir;

/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The unit documents' directory, relative to the repository root.
const CP_DIR: &str = "crates/sacm/tests/data/sacm23/cp";

/// What a walk of one unit document saw.
#[derive(Debug, Default)]
struct Seen {
    /// The classes of the objects seen.
    classes: BTreeSet<String>,
    /// The properties seen, as (declaring class, property name).
    properties: BTreeSet<(String, String)>,
    /// The values of the `assertionDeclaration` attributes seen.
    assertion_declarations: BTreeSet<String>,
    /// How many objects of an `Assertion` class carry no
    /// `assertionDeclaration` attribute.
    assertions_without_declaration: usize,
}

/// The pinned model with the recorded corrections, or `None` after the
/// SKIPPED notice.
fn corrected_model(row: &str) -> Option<Metamodel> {
    let dir = spec_dir(row, &[MODEL_FILE])?;
    let raw = load(&dir).unwrap_or_else(|e| panic!("{row}: load({}) fails: {e}", dir.display()));
    Some(
        raw.corrected()
            .unwrap_or_else(|e| panic!("{row}: corrected() of the pinned model fails: {e}")),
    )
}

/// The local part of a qualified name (`sacm:Claim` gives `Claim`).
fn local_part(qname: &str) -> &str {
    qname.rsplit(':').next().unwrap_or(qname)
}

/// Whether `class` is `Assertion` or specializes it.
fn is_assertion(model: &Metamodel, class: &str) -> bool {
    class == "Assertion" || model.ancestors(class).contains(&"Assertion")
}

/// Walks the object `node` of class `class`, at `path` in the document.
fn visit(
    row: &str,
    model: &Metamodel,
    node: roxmltree::Node<'_, '_>,
    class: &str,
    path: &str,
    seen: &mut Seen,
) {
    let Some(model_class) = model.class(class) else {
        panic!("{row}: {path}: the class {class} is not in the model");
    };
    assert!(
        !model_class.is_abstract,
        "{row}: {path}: the object's class {class} is abstract"
    );
    seen.classes.insert(class.to_owned());
    let properties = model.properties_of(class);
    let lookup = |name: &str| {
        properties
            .iter()
            .find(|(_, property)| property.name == name)
            .copied()
    };

    for attribute in node.attributes().filter(|a| a.namespace().is_none()) {
        let name = attribute.name();
        let Some((declaring, _)) = lookup(name) else {
            panic!("{row}: {path}: the attribute {name} is no property of {class}");
        };
        seen.properties
            .insert((declaring.to_owned(), name.to_owned()));
        if name == "assertionDeclaration" {
            seen.assertion_declarations
                .insert(attribute.value().to_owned());
        }
    }
    if is_assertion(model, class) && node.attribute("assertionDeclaration").is_none() {
        seen.assertions_without_declaration += 1;
    }

    for (index, child) in node
        .children()
        .filter(roxmltree::Node::is_element)
        .enumerate()
    {
        let tag = child.tag_name();
        if tag.namespace() == Some(XMI_NS) {
            continue;
        }
        let name = tag.name();
        let child_path = format!("{path}/{name}[{index}]");
        assert!(
            tag.namespace().is_none(),
            "{row}: {child_path}: the child is in the namespace {:?}, not a property element",
            tag.namespace()
        );
        let Some((declaring, property)) = lookup(name) else {
            panic!("{row}: {child_path}: the child {name} is no property of {class}");
        };
        seen.properties
            .insert((declaring.to_owned(), name.to_owned()));
        let is_link =
            child.attribute("href").is_some() || child.attribute((XMI_NS, "idref")).is_some();
        if property.kind == PropertyKind::Composition && !is_link {
            let child_class = child
                .attribute((XMI_NS, "type"))
                .map_or(property.type_name.as_str(), local_part);
            visit(row, model, child, child_class, &child_path, seen);
        }
    }
}

/// Walks the unit document `file` of `row` against `model`.
fn walk(row: &str, model: &Metamodel, file: &str) -> Seen {
    let path = xtask::repo_root().join(CP_DIR).join(file);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{row}: setup: read {}: {e}", path.display()));
    let document = roxmltree::Document::parse(&text)
        .unwrap_or_else(|e| panic!("{row}: {file} does not parse: {e}"));
    let root = document.root_element();
    let class = root.tag_name().name();
    let mut seen = Seen::default();
    visit(row, model, root, class, class, &mut seen);
    seen
}

/// Checks that the walk of `file` saw, for each of `packages` (a package
/// name and its number of concrete classes), every concrete class (and that
/// the package has that many) and every property its classes declare.
/// Returns the walk, or `None` when the model is absent and the test skips.
fn check_unit_document(
    row: &str,
    file: &str,
    packages: &[(&str, usize)],
) -> Option<(Metamodel, Seen)> {
    let model = corrected_model(row)?;
    let seen = walk(row, &model, file);
    for &(name, concrete) in packages {
        let Some(package) = model.packages.iter().find(|p| p.name == name) else {
            panic!("{row}: the model has no package {name}");
        };
        let classes: Vec<&str> = package
            .classes
            .iter()
            .filter(|c| !c.is_abstract)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(
            classes.len(),
            concrete,
            "{row}: the package {name} has {} concrete classes, not {concrete}: {classes:?}",
            classes.len()
        );
        let unseen: Vec<&str> = classes
            .iter()
            .copied()
            .filter(|c| !seen.classes.contains(*c))
            .collect();
        assert!(
            unseen.is_empty(),
            "{row}: {file} holds no object of the concrete classes {unseen:?} of {name}"
        );
        let unseen_properties: Vec<String> = package
            .classes
            .iter()
            .flat_map(|c| {
                c.properties
                    .iter()
                    .map(move |p| (c.name.as_str(), p.name.as_str()))
            })
            .filter(|&(c, p)| !seen.properties.contains(&(c.to_owned(), p.to_owned())))
            .map(|(c, p)| format!("{c}.{p}"))
            .collect();
        assert!(
            unseen_properties.is_empty(),
            "{row}: {file} uses none of the properties {unseen_properties:?} of {name}"
        );
    }
    Some((model, seen))
}

#[test]
fn sacm23_cp_001_the_assurance_case_unit_document_holds_every_class_and_property_of_the_model() {
    const ROW: &str = "SACM23-CP-001";
    let packages = [
        ("Base", 7),
        ("AssuranceCase", 3),
        ("Artifact", 12),
        ("Argumentation", 12),
        ("Terminology", 7),
    ];
    let Some((model, _)) = check_unit_document(ROW, "assurance-case-unit.sacm.xmi", &packages)
    else {
        return;
    };
    let names: Vec<&str> = model.packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Base",
            "AssuranceCase",
            "Artifact",
            "Argumentation",
            "Terminology"
        ],
        "{ROW}: the whole model is the five packages walked"
    );
    let concrete: usize = packages.iter().map(|&(_, n)| n).sum();
    assert_eq!(
        concrete, 41,
        "{ROW}: the whole model has 41 concrete classes"
    );
    let properties: usize = model
        .packages
        .iter()
        .flat_map(|p| p.classes.iter())
        .map(|c| c.properties.len())
        .sum();
    assert_eq!(
        properties, 61,
        "{ROW}: the whole model declares 61 properties (60 in ptc/22-03-13, plus \
         Resource.location)"
    );
}

#[test]
fn sacm23_cp_002_the_argumentation_unit_document_holds_every_class_and_property_of_the_model() {
    const ROW: &str = "SACM23-CP-002";
    let Some((model, seen)) = check_unit_document(
        ROW,
        "argumentation-unit.sacm.xmi",
        &[("Argumentation", 12), ("Base", 7)],
    ) else {
        return;
    };
    let named = [
        "needsSupport",
        "assumed",
        "axiomatic",
        "defeated",
        "asCited",
    ];
    let missing: Vec<&str> = named
        .iter()
        .copied()
        .filter(|literal| !seen.assertion_declarations.contains(*literal))
        .collect();
    assert!(
        missing.is_empty(),
        "{ROW}: argumentation-unit.sacm.xmi has no assertionDeclaration {missing:?}"
    );
    assert!(
        seen.assertions_without_declaration >= 1,
        "{ROW}: argumentation-unit.sacm.xmi has no Assertion without an assertionDeclaration \
         (the default, asserted)"
    );
    let literals: Vec<&str> = model
        .packages
        .iter()
        .flat_map(|p| p.enumerations.iter())
        .find(|e| e.name == "AssertionDeclaration")
        .map(|e| e.literals.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let mut expected: Vec<&str> = vec!["asserted"];
    expected.extend(named);
    assert_eq!(
        literals, expected,
        "{ROW}: every AssertionDeclaration literal is the default or one the document names"
    );
}

#[test]
fn sacm23_cp_003_the_artifact_unit_document_holds_every_class_and_property_of_the_model() {
    check_unit_document(
        "SACM23-CP-003",
        "artifact-unit.sacm.xmi",
        &[("Artifact", 12), ("Base", 7)],
    );
}

#[test]
fn sacm23_cp_004_the_terminology_unit_document_holds_every_class_and_property_of_the_model() {
    check_unit_document(
        "SACM23-CP-004",
        "terminology-unit.sacm.xmi",
        &[("Terminology", 7), ("Base", 7)],
    );
}
