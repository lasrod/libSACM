//! Metamodel coverage against ptc/22-03-13 (LIB-TOOL-004).
//!
//! `xtask::metamodel` reads the pinned OMG model, `SACM-2.3-ptc-22-03-13.xml`,
//! and applies the five recorded corrections (D8, D19, D17, D5 and §11.3).
//! These tests compare what it reads with the crate's metadata (`ElementKind`,
//! `AbstractClass`, `ElementKind::may_contain`, `Flag`, `ReferenceEnd`), with
//! what a strict read and write of a document keep, and with the hand-kept
//! `docs/sacm/metamodel-inventory.md`, so that none of them can drift from
//! the model unnoticed (M6 decisions Q10). There is no inventory generator.
//!
//! Tests 1 and 2 use the design brief's sample model and need nothing else.
//! The others need the pinned model: see `common/spec.rs` for where it is
//! found and when a test is skipped.

#[path = "common/sample.rs"]
mod sample;
#[path = "common/spec.rs"]
mod spec;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use sacm::diagnostic::Severity;
use sacm::model::{AbstractClass, ElementId, ElementKind, Flag, MetamodelPackage, ReferenceEnd};
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};
use xtask::fetch::{ARTIFACTS, sha256_hex};
use xtask::metamodel::{
    CORRECTIONS, Class, Enumeration, MODEL_FILE, Metamodel, Package, Property, PropertyKind, load,
    parse,
};

use sample::SAMPLE_MODEL;
use spec::spec_dir;

const ROW: &str = "LIB-TOOL-004";

/// The `MagicDraw` `Boolean` the model types `isCitation`, `isAbstract` and
/// `isCounter` by (`UML_Standard_Profile.mdzip#…`, spec defect D14).
const BOOLEAN: &str = "eee_1045467100323_191782_59";

/// The undefined `MagicDraw` type of the four dates (spec defect D17).
const DATE: &str = "_be00301_1073305590699_364818_1";

/// The three value types: concrete classes of the model that are no
/// `SACMElement`, so no `ElementKind`.
const VALUE_TYPES: [&str; 3] = ["LangString", "MultiLangString", "ExpressionLangString"];

/// The inventory, relative to the repository root.
const INVENTORY: &str = "docs/sacm/metamodel-inventory.md";

/// The SACM and XMI namespace declarations of a test document.
const NS: &str = r#"xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001""#;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A fresh directory under the system temporary directory, removed on drop.
#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn new(test: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path =
            std::env::temp_dir().join(format!("xtask-{test}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path)
            .unwrap_or_else(|e| panic!("{ROW}: setup: create {}: {e}", path.display()));
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `xml` parsed, or the test fails naming what.
fn parsed(what: &str, xml: &str) -> Metamodel {
    parse(xml).unwrap_or_else(|e| panic!("{ROW}: {what} does not parse: {e}"))
}

/// The pinned model as written, or `None` after the SKIPPED notice.
fn raw_model() -> Option<Metamodel> {
    let dir = spec_dir(ROW, &[MODEL_FILE])?;
    Some(load(&dir).unwrap_or_else(|e| panic!("{ROW}: load({}) fails: {e}", dir.display())))
}

/// The pinned model with the five corrections, or `None` after the SKIPPED
/// notice.
fn corrected_model() -> Option<Metamodel> {
    let raw = raw_model()?;
    Some(
        raw.corrected()
            .unwrap_or_else(|e| panic!("{ROW}: corrected() of the pinned model fails: {e}")),
    )
}

/// A property with no association.
fn property(
    name: &str,
    kind: PropertyKind,
    type_name: &str,
    lower: u32,
    upper: Option<u32>,
) -> Property {
    Property {
        name: name.to_owned(),
        association: None,
        kind,
        type_name: type_name.to_owned(),
        lower,
        upper,
    }
}

/// A class.
fn class(
    name: &str,
    is_abstract: bool,
    superclass: Option<&str>,
    properties: Vec<Property>,
) -> Class {
    Class {
        name: name.to_owned(),
        is_abstract,
        superclass: superclass.map(str::to_owned),
        properties,
    }
}

/// Every class of `model`, in document order.
fn classes(model: &Metamodel) -> impl Iterator<Item = &Class> {
    model.packages.iter().flat_map(|p| p.classes.iter())
}

/// `n` as a `usize`.
fn to_usize(n: u32) -> usize {
    usize::try_from(n).unwrap_or_else(|_| panic!("{ROW}: setup: {n} is no usize"))
}

// ---------------------------------------------------------------------------
// 1 and 2: parse (no spec needed)
// ---------------------------------------------------------------------------

/// The sample model as `parse` must read it: document order, bounds as
/// written, the href fragment as a Data property's type name.
fn sample_value() -> Metamodel {
    use PropertyKind::{Composition, Data, Enumeration as Enum, Reference};
    Metamodel {
        packages: vec![Package {
            name: "Sample".to_owned(),
            classes: vec![
                class(
                    "Item",
                    true,
                    None,
                    vec![property("label", Data, "String", 0, Some(1))],
                ),
                class(
                    "Leaf",
                    false,
                    Some("Item"),
                    vec![
                        property("state", Enum, "State", 1, Some(1)),
                        property("next", Reference, "Item", 0, Some(1)),
                        property("item", Reference, "Item", 0, Some(1)),
                    ],
                ),
                class(
                    "Box",
                    false,
                    Some("Item"),
                    vec![
                        property("item", Composition, "Item", 0, None),
                        property("tag", Composition, "Tag", 0, Some(1)),
                        property("next", Reference, "Leaf", 2, None),
                    ],
                ),
                class("Tag", false, None, vec![]),
            ],
            enumerations: vec![Enumeration {
                name: "State".to_owned(),
                literals: vec!["open".to_owned(), "closed".to_owned()],
            }],
        }],
    }
}

/// Two associations in the shape of ptc/22-03-13: a named one whose
/// class-owned end has no name and names it (`association=`), and an unnamed
/// one whose class-owned end is named and is linked only by `memberEnd`.
const ASSOCIATION_MODEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <uml:Model xmi:type="uml:Model" xmi:id="m" name="Model">
    <packagedElement xmi:type="uml:Package" xmi:id="p" name="Links">
      <packagedElement xmi:type="uml:Class" xmi:id="A" name="A">
        <ownedAttribute xmi:type="uml:Property" xmi:id="A.owner" type="B" association="AB"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="A.peer" name="peer" type="B"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="B" name="B"/>
      <packagedElement xmi:type="uml:Association" xmi:id="AB" name="owner">
        <memberEnd xmi:idref="A.owner"/>
        <memberEnd xmi:idref="AB.end"/>
        <ownedEnd xmi:type="uml:Property" xmi:id="AB.end" type="A" association="AB"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Association" xmi:id="AB2">
        <memberEnd xmi:idref="A.peer"/>
        <memberEnd xmi:idref="AB2.end"/>
        <ownedEnd xmi:type="uml:Property" xmi:id="AB2.end" type="A" association="AB2"/>
      </packagedElement>
    </packagedElement>
  </uml:Model>
</xmi:XMI>
"#;

#[test]
fn lib_tool_004_parse_reads_classes_properties_bounds_and_enumerations() {
    let model = parsed("the sample model", SAMPLE_MODEL);
    assert_eq!(
        model,
        sample_value(),
        "{ROW}: parse(SAMPLE_MODEL) is not the sample's Metamodel value"
    );

    // The accessors over it.
    assert_eq!(
        model.class("Leaf").map(|c| c.name.as_str()),
        Some("Leaf"),
        "{ROW}: class(Leaf)"
    );
    assert!(model.class("Nope").is_none(), "{ROW}: class(Nope) is None");
    assert_eq!(
        model.package_of("Box"),
        Some("Sample"),
        "{ROW}: package_of(Box)"
    );
    assert_eq!(model.package_of("Nope"), None, "{ROW}: package_of(Nope)");
    assert_eq!(model.ancestors("Leaf"), ["Item"], "{ROW}: ancestors(Leaf)");
    assert!(
        model.ancestors("Tag").is_empty(),
        "{ROW}: Tag has no ancestor"
    );
    let leaf: Vec<(&str, &str)> = model
        .properties_of("Leaf")
        .into_iter()
        .map(|(declaring, p)| (declaring, p.name.as_str()))
        .collect();
    assert_eq!(
        leaf,
        [
            ("Item", "label"),
            ("Leaf", "state"),
            ("Leaf", "next"),
            ("Leaf", "item")
        ],
        "{ROW}: properties_of(Leaf) gives (declaring class, property), inherited first, each \
         class's own in model order"
    );

    // Associations: a property that is an end of one carries its name, ""
    // for an unnamed one; the association-owned ends are no class property.
    let links = parsed("the association model", ASSOCIATION_MODEL);
    let a = links
        .class("A")
        .unwrap_or_else(|| panic!("{ROW}: class A is read"));
    let ends: Vec<(&str, Option<&str>)> = a
        .properties
        .iter()
        .map(|p| (p.name.as_str(), p.association.as_deref()))
        .collect();
    assert_eq!(
        ends,
        [("", Some("owner")), ("peer", Some(""))],
        "{ROW}: an association end is read with its association's name, \"\" when unnamed, and \
         an unnamed end keeps the name \"\""
    );
    assert!(
        links.class("B").is_some_and(|b| b.properties.is_empty()),
        "{ROW}: an Association's ownedEnd is not a property of its type"
    );
}

/// A small model that parses; each refusal below changes one thing in it.
const BASE_MODEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <uml:Model xmi:type="uml:Model" xmi:id="m" name="Model">
    <packagedElement xmi:type="uml:Package" xmi:id="p" name="Sample">
      <packagedElement xmi:type="uml:Class" xmi:id="Item" name="Item" isAbstract="true"/>
      <packagedElement xmi:type="uml:Class" xmi:id="Tag" name="Tag"/>
      <packagedElement xmi:type="uml:Class" xmi:id="Leaf" name="Leaf">
        <generalization xmi:type="uml:Generalization" xmi:id="Leaf.general" general="Item"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Leaf.next" name="next" type="Item"/>
      </packagedElement>
      <!-- more -->
    </packagedElement>
  </uml:Model>
</xmi:XMI>
"#;

/// [`BASE_MODEL`] with `from` replaced by `to`, which must occur in it.
fn base_with(from: &str, to: &str) -> String {
    assert!(
        BASE_MODEL.contains(from),
        "{ROW}: setup: {from:?} is not in the base model"
    );
    BASE_MODEL.replacen(from, to, 1)
}

#[test]
fn lib_tool_004_parse_refuses_what_it_does_not_model() {
    // Controls: the base model, with an Association and an Enumeration too.
    parsed("the base model", BASE_MODEL);
    parsed(
        "the base model with an Association and an Enumeration",
        &base_with(
            "<!-- more -->",
            r#"<packagedElement xmi:type="uml:Association" xmi:id="as" name="link"><memberEnd xmi:idref="Leaf.next"/></packagedElement>
      <packagedElement xmi:type="uml:Enumeration" xmi:id="E" name="E"><ownedLiteral xmi:type="uml:EnumerationLiteral" xmi:id="E.a" name="a"/></packagedElement>"#,
        ),
    );

    let no_model = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <uml:Package xmi:type="uml:Package" xmi:id="p" name="Sample"/>
</xmi:XMI>
"#;
    let cases: [(&str, String, &str); 6] = [
        ("no uml:Model", no_model.to_owned(), "uml:Model"),
        (
            "a nested package",
            base_with(
                "<!-- more -->",
                r#"<packagedElement xmi:type="uml:Package" xmi:id="inner" name="Inner"/>"#,
            ),
            "Inner",
        ),
        (
            "a packagedElement that is no Class, Enumeration or Association",
            base_with(
                "<!-- more -->",
                r#"<packagedElement xmi:type="uml:DataType" xmi:id="dt" name="Stamp"/>"#,
            ),
            "uml:DataType",
        ),
        (
            "a class with two generalizations",
            base_with(
                r#"general="Item"/>"#,
                r#"general="Item"/><generalization xmi:type="uml:Generalization" xmi:id="Leaf.general2" general="Tag"/>"#,
            ),
            "Leaf",
        ),
        (
            "an unresolved idref",
            base_with(r#"name="next" type="Item""#, r#"name="next" type="Nope""#),
            "Nope",
        ),
        (
            "a duplicate class name",
            base_with(
                "<!-- more -->",
                r#"<packagedElement xmi:type="uml:Class" xmi:id="Leaf2" name="Leaf"/>"#,
            ),
            "Leaf",
        ),
    ];
    for (what, xml, named) in &cases {
        match parse(xml) {
            Ok(model) => panic!("{ROW}: parse accepts {what}, giving {model:?}"),
            Err(message) => assert!(
                message.contains(named),
                "{ROW}: the refusal of {what} names {named:?}, got {message:?}"
            ),
        }
    }
}

/// A package nested in a package is refused as nested, naming both, and not
/// as an element of another kind.
#[test]
fn lib_tool_004_parse_refuses_a_nested_package_as_nested() {
    let xml = base_with(
        "<!-- more -->",
        r#"<packagedElement xmi:type="uml:Package" xmi:id="inner" name="Inner"/>"#,
    );
    match parse(&xml) {
        Ok(model) => panic!("{ROW}: a nested package parses: {model:?}"),
        Err(message) => assert_eq!(
            message, "the package Inner is nested in the package Sample; packages are not nested",
            "{ROW}: the refusal of a nested package"
        ),
    }
}

// ---------------------------------------------------------------------------
// 3 and 4: the pinned model and its corrections
// ---------------------------------------------------------------------------

/// Junk that is not the pinned model.
const JUNK: &[u8] = b"junk: not the pinned ptc/22-03-13";

/// `load` refuses a missing or changed model, naming the fix.
fn assert_load_refuses_a_missing_or_changed_model() {
    let pin = ARTIFACTS
        .iter()
        .find(|a| a.file_name == MODEL_FILE)
        .map_or_else(
            || panic!("{ROW}: {MODEL_FILE} is in ARTIFACTS"),
            |a| a.sha256,
        );
    let dir = TempDir::new("lib-tool-004-load");
    match load(dir.path()) {
        Ok(_) => panic!("{ROW}: load of a directory without {MODEL_FILE} succeeds"),
        Err(message) => assert!(
            message.contains(MODEL_FILE) && message.contains("cargo xtask fetch-spec"),
            "{ROW}: load names the missing {MODEL_FILE} and `cargo xtask fetch-spec`, got \
             {message:?}"
        ),
    }
    fs::write(dir.path().join(MODEL_FILE), JUNK)
        .unwrap_or_else(|e| panic!("{ROW}: setup: write {MODEL_FILE}: {e}"));
    match load(dir.path()) {
        Ok(_) => panic!("{ROW}: load of a changed {MODEL_FILE} succeeds"),
        Err(message) => assert!(
            message.contains(&sha256_hex(JUNK))
                && message.contains(pin)
                && message.contains("cargo xtask fetch-spec"),
            "{ROW}: load names the actual and pinned SHA-256 and `cargo xtask fetch-spec`, got \
             {message:?}"
        ),
    }
}

#[test]
fn lib_tool_004_the_pinned_model_has_its_known_shape() {
    assert_eq!(MODEL_FILE, "SACM-2.3-ptc-22-03-13.xml", "{ROW}: MODEL_FILE");
    assert_load_refuses_a_missing_or_changed_model();
    let Some(model) = raw_model() else { return };

    let packages: Vec<(&str, usize)> = model
        .packages
        .iter()
        .map(|p| (p.name.as_str(), p.classes.len()))
        .collect();
    assert_eq!(
        packages,
        [
            ("Base", 12),
            ("AssuranceCase", 3),
            ("Artifact", 13),
            ("Argumentation", 16),
            ("Terminology", 10)
        ],
        "{ROW}: the packages in document order, with their class counts (54)"
    );
    assert_eq!(
        classes(&model).filter(|c| c.is_abstract).count(),
        12,
        "{ROW}: 12 abstract classes"
    );

    let properties: Vec<(&str, &str, &Property)> = model
        .packages
        .iter()
        .flat_map(|p| p.classes.iter().map(move |c| (p, c)))
        .flat_map(|(p, c)| {
            c.properties
                .iter()
                .map(move |q| (p.name.as_str(), c.name.as_str(), q))
        })
        .collect();
    assert_eq!(properties.len(), 60, "{ROW}: 60 properties");
    let unnamed: Vec<(&str, &str)> = properties
        .iter()
        .filter(|(_, _, p)| p.name.is_empty())
        .map(|&(package, class, _)| (package, class))
        .collect();
    assert_eq!(
        unnamed.len(),
        11,
        "{ROW}: 11 unnamed properties: {unnamed:?}"
    );
    assert!(
        unnamed.iter().all(|&(package, _)| package == "Base"),
        "{ROW}: every unnamed property is in Base: {unnamed:?}"
    );
    let ends: Vec<&Property> = properties
        .iter()
        .map(|&(_, _, p)| p)
        .filter(|p| p.association.is_some())
        .collect();
    assert_eq!(
        ends.len(),
        46,
        "{ROW}: 46 associations, one class-owned end each"
    );
    let named: Vec<&&Property> = ends
        .iter()
        .filter(|p| p.association.as_deref().is_some_and(|a| !a.is_empty()))
        .collect();
    assert_eq!(named.len(), 11, "{ROW}: 11 associations are named");
    assert!(
        named.iter().all(|p| p.name.is_empty()),
        "{ROW}: the 11 named associations are those of the 11 unnamed properties"
    );

    let enumerations: Vec<(&str, &Enumeration)> = model
        .packages
        .iter()
        .flat_map(|p| p.enumerations.iter().map(move |e| (p.name.as_str(), e)))
        .collect();
    let [(package, enumeration)] = enumerations.as_slice() else {
        panic!("{ROW}: exactly one enumeration, got {enumerations:?}");
    };
    assert_eq!(
        (
            *package,
            enumeration.name.as_str(),
            enumeration.literals.as_slice()
        ),
        (
            "Argumentation",
            "AssertionDeclaration",
            &[
                "asserted",
                "needsSupport",
                "assumed",
                "axiomatic",
                "defeated",
                "asCited"
            ]
            .map(String::from)[..]
        ),
        "{ROW}: the enumeration AssertionDeclaration with its six literals in order"
    );
    // A class with two generalizations is refused by parse (test 2), so the
    // model read means none has two.
}

/// `raw` with the five corrections applied as the design brief states them,
/// independently of `corrected()`.
fn corrected_by_hand(raw: &Metamodel) -> Metamodel {
    let mut model = raw.clone();
    let mut seen = [false; 5];
    for class in model.packages.iter_mut().flat_map(|p| p.classes.iter_mut()) {
        // D8: an unnamed property takes its association's name.
        for p in &mut class.properties {
            if p.name.is_empty() {
                p.name = p.association.clone().unwrap_or_else(|| {
                    panic!(
                        "{ROW}: an unnamed property of {} has no association",
                        class.name
                    )
                });
                seen[0] = true;
            }
        }
        match class.name.as_str() {
            "ArtifactAssertedRelationship" => {
                "ArtifactAssetRelationship".clone_into(&mut class.name);
                class.superclass = Some("ArtifactAsset".to_owned());
                seen[1] = true;
            }
            "Event" => {
                for p in class.properties.iter_mut().filter(|p| p.name == "occurece") {
                    "date".clone_into(&mut p.name);
                    seen[2] = true;
                }
            }
            "Resource" => {
                class.properties.push(property(
                    "location",
                    PropertyKind::Composition,
                    "MultiLangString",
                    0,
                    Some(1),
                ));
                seen[3] = true;
            }
            "ArgumentationElement" => {
                class.is_abstract = true;
                seen[4] = true;
            }
            _ => {}
        }
    }
    assert_eq!(
        seen, [true; 5],
        "{ROW}: every correction has its subject in the raw model"
    );
    model
}

#[test]
fn lib_tool_004_the_corrections_are_the_five_recorded_ones() {
    let ids: Vec<&str> = CORRECTIONS.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        ["D8", "D19", "D17", "D5", "§11.3"],
        "{ROW}: the CORRECTIONS ids in order"
    );
    for correction in CORRECTIONS {
        assert!(
            !correction.summary.trim().is_empty(),
            "{ROW}: correction {} has a summary",
            correction.id
        );
    }

    // A model holding the subjects of D19, D17, D5 and §11.3 but no unnamed
    // property: D8's subject alone is missing, so corrected() names D8.
    let without_d8 = |properties: Vec<Property>| Metamodel {
        packages: vec![Package {
            name: "Subjects".to_owned(),
            classes: vec![
                class("ArtifactAsset", true, None, vec![]),
                class("ArtifactAssertedRelationship", false, None, vec![]),
                class(
                    "Event",
                    false,
                    None,
                    vec![property("occurece", PropertyKind::Data, DATE, 0, Some(1))],
                ),
                class("Resource", false, None, vec![]),
                class("ArgumentationElement", false, None, properties),
            ],
            enumerations: vec![],
        }],
    };
    match without_d8(vec![]).corrected() {
        Ok(model) => {
            panic!("{ROW}: corrected() of a model with no unnamed property succeeds: {model:?}")
        }
        Err(message) => assert!(
            message.contains("D8"),
            "{ROW}: corrected() of a model with no unnamed property names D8, got {message:?}"
        ),
    }
    // Control: with one unnamed property of a named Association, every
    // correction has its subject.
    let mut unnamed = property("", PropertyKind::Reference, "ArtifactAsset", 0, None);
    unnamed.association = Some("link".to_owned());
    assert!(
        without_d8(vec![unnamed]).corrected().is_ok(),
        "{ROW}: the same model with an unnamed property of a named Association corrects"
    );

    let Some(raw) = raw_model() else { return };
    let corrected = raw
        .corrected()
        .unwrap_or_else(|e| panic!("{ROW}: corrected() of the pinned model fails: {e}"));
    assert_eq!(
        corrected,
        corrected_by_hand(&raw),
        "{ROW}: corrected() differs from the raw model in exactly the five recorded ways"
    );
    assert_eq!(
        classes(&corrected).filter(|c| c.is_abstract).count(),
        13,
        "{ROW}: 13 abstract"
    );
    assert_eq!(
        classes(&corrected).filter(|c| !c.is_abstract).count(),
        41,
        "{ROW}: 41 concrete"
    );
    assert!(
        classes(&corrected).all(|c| c.properties.iter().all(|p| !p.name.is_empty())),
        "{ROW}: no property of the corrected model is unnamed"
    );
    match corrected.corrected() {
        Ok(_) => {
            panic!("{ROW}: corrected() of a corrected model succeeds, but its subjects are gone")
        }
        Err(message) => assert!(
            ids.iter().any(|id| message.contains(id)),
            "{ROW}: corrected() names the correction whose subject is absent, got {message:?}"
        ),
    }
}

/// The subjects of the five corrections, with the unnamed property
/// `unnamed` of `ArgumentationElement`, the class `ArtifactAsset` if
/// `artifact_asset`, and `resource` as the properties of `Resource`.
fn subjects(unnamed: Property, artifact_asset: bool, resource: Vec<Property>) -> Metamodel {
    let mut classes = vec![
        class("ArtifactAssertedRelationship", false, None, vec![]),
        class(
            "Event",
            false,
            None,
            vec![property("occurece", PropertyKind::Data, DATE, 0, Some(1))],
        ),
        class("Resource", false, None, resource),
        class("ArgumentationElement", false, None, vec![unnamed]),
    ];
    if artifact_asset {
        classes.insert(0, class("ArtifactAsset", true, None, vec![]));
    }
    Metamodel {
        packages: vec![Package {
            name: "Subjects".to_owned(),
            classes,
            enumerations: vec![],
        }],
    }
}

/// An unnamed Reference property of the Association `association`.
fn unnamed_of(association: &str) -> Property {
    let mut unnamed = property("", PropertyKind::Reference, "ArtifactAsset", 0, None);
    unnamed.association = Some(association.to_owned());
    unnamed
}

/// `corrected()` refuses a subject in a shape its correction cannot apply
/// to, naming the correction and what is wrong: an unnamed property whose
/// Association is unnamed too (D8), no `ArtifactAsset` (D19), and a
/// `Resource` that already has `location` (D5). A `Resource` with other
/// properties is corrected.
#[test]
fn lib_tool_004_corrected_refuses_a_subject_its_correction_cannot_apply_to() {
    let other = || property("other", PropertyKind::Data, "String", 0, Some(1));
    let located = || property("location", PropertyKind::Data, "String", 0, Some(1));
    for (what, model, refusal) in [
        (
            "an unnamed property of an unnamed Association",
            subjects(unnamed_of(""), true, vec![]),
            "correction D8 (",
        ),
        (
            "no ArtifactAsset",
            subjects(unnamed_of("link"), false, vec![]),
            "class ArtifactAsset is absent",
        ),
        (
            "a Resource that already has location",
            subjects(unnamed_of("link"), true, vec![other(), located()]),
            "Resource already has location",
        ),
    ] {
        match model.corrected() {
            Ok(corrected) => {
                panic!("{ROW}: corrected() of a model with {what} succeeds: {corrected:?}")
            }
            Err(message) => assert!(
                message.contains(refusal),
                "{ROW}: corrected() of a model with {what} says {refusal:?}, got {message:?}"
            ),
        }
    }
    match subjects(unnamed_of(""), true, vec![]).corrected() {
        Err(message) => assert!(
            message
                .contains("an unnamed property of ArgumentationElement has no named Association"),
            "{ROW}: the D8 refusal names the class, got {message:?}"
        ),
        Ok(corrected) => panic!("{ROW}: an unnamed Association is corrected: {corrected:?}"),
    }
    // Control: a Resource with another property gets `location`.
    let corrected = subjects(unnamed_of("link"), true, vec![other()])
        .corrected()
        .unwrap_or_else(|e| panic!("{ROW}: a Resource with another property is refused: {e}"));
    assert_eq!(
        corrected.class("Resource").map(|c| c
            .properties
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()),
        Some(vec!["other", "location"]),
        "{ROW}: D5 adds location after the Resource's own properties"
    );
}

/// `ancestors` walks the superclasses nearest first, through a superclass
/// the model does not hold, and stops before a class seen already, the
/// class itself included.
#[test]
fn lib_tool_004_ancestors_stop_before_a_class_seen_already() {
    let model = |classes: Vec<Class>| Metamodel {
        packages: vec![Package {
            name: "P".to_owned(),
            classes,
            enumerations: vec![],
        }],
    };
    // First, as it ends whatever the stop: a class that is its own
    // superclass has no ancestor.
    let own = model(vec![class("A", false, Some("A"), vec![])]);
    assert_eq!(
        own.ancestors("A"),
        Vec::<&str>::new(),
        "{ROW}: the ancestors of A, its own superclass"
    );
    for (what, classes, expected) in [
        (
            "A, B, C",
            vec![
                class("A", false, Some("B"), vec![]),
                class("B", false, Some("C"), vec![]),
                class("C", false, None, vec![]),
            ],
            vec!["B", "C"],
        ),
        (
            "A under the absent Nope",
            vec![class("A", false, Some("Nope"), vec![])],
            vec!["Nope"],
        ),
        (
            "A and B each other's superclass",
            vec![
                class("A", false, Some("B"), vec![]),
                class("B", false, Some("A"), vec![]),
            ],
            vec!["B"],
        ),
        (
            "A under B and C, each other's superclass",
            vec![
                class("A", false, Some("B"), vec![]),
                class("B", false, Some("C"), vec![]),
                class("C", false, Some("B"), vec![]),
            ],
            vec!["B", "C"],
        ),
    ] {
        assert_eq!(
            model(classes).ancestors("A"),
            expected,
            "{ROW}: the ancestors of A in {what}"
        );
    }
}

/// `load` reports a model it cannot read as a read error naming the file,
/// not as missing: here the model's path is a directory.
#[test]
fn lib_tool_004_load_reports_an_unreadable_model_as_a_read_error() {
    let dir = TempDir::new("lib-tool-004-unreadable");
    fs::create_dir(dir.path().join(MODEL_FILE))
        .unwrap_or_else(|e| panic!("{ROW}: setup: create the directory {MODEL_FILE}: {e}"));
    match load(dir.path()) {
        Ok(_) => panic!("{ROW}: load of a directory named {MODEL_FILE} succeeds"),
        Err(message) => assert!(
            message.starts_with("read ")
                && message.contains(MODEL_FILE)
                && !message.contains("is missing"),
            "{ROW}: load reports a read error naming {MODEL_FILE}, got {message:?}"
        ),
    }
}

// ---------------------------------------------------------------------------
// 5 to 9: the crate's metadata
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_004_every_class_is_a_kind_an_abstract_class_or_a_value_type() {
    let Some(model) = corrected_model() else {
        return;
    };
    let package = |name: &str| {
        model
            .package_of(name)
            .unwrap_or_else(|| panic!("{ROW}: package_of({name})"))
            .to_owned()
    };
    let concrete: BTreeSet<(String, String)> = classes(&model)
        .filter(|c| !c.is_abstract && !VALUE_TYPES.contains(&c.name.as_str()))
        .map(|c| (c.name.clone(), package(&c.name)))
        .collect();
    let kinds: BTreeSet<(String, String)> = ElementKind::ALL
        .iter()
        .map(|k| (k.name().to_owned(), k.package().name().to_owned()))
        .collect();
    assert_eq!(
        concrete, kinds,
        "{ROW}: the concrete classes are ElementKind::ALL by (name, package)"
    );
    assert_eq!(kinds.len(), 38, "{ROW}: 38 kinds");
    for value in VALUE_TYPES {
        assert!(
            model.class(value).is_some_and(|c| !c.is_abstract)
                && model.package_of(value) == Some(MetamodelPackage::Base.name()),
            "{ROW}: {value} is a concrete class of Base"
        );
    }
    let abstract_classes: BTreeSet<(String, String)> = classes(&model)
        .filter(|c| c.is_abstract)
        .map(|c| (c.name.clone(), package(&c.name)))
        .collect();
    let crate_abstract: BTreeSet<(String, String)> = AbstractClass::ALL
        .iter()
        .map(|a| (a.name().to_owned(), a.package().name().to_owned()))
        .collect();
    assert_eq!(
        abstract_classes, crate_abstract,
        "{ROW}: the abstract classes are AbstractClass::ALL by (name, package)"
    );
    assert_eq!(crate_abstract.len(), 13, "{ROW}: 13 abstract classes");
}

#[test]
fn lib_tool_004_generalizations_agree_with_is_a_and_is_kind_of() {
    let Some(model) = corrected_model() else {
        return;
    };
    assert_eq!(
        model.ancestors("Claim"),
        [
            "Assertion",
            "ArgumentAsset",
            "ArgumentationElement",
            "ArtifactElement",
            "ModelElement",
            "SACMElement",
            "Element"
        ],
        "{ROW}: ancestors(Claim), nearest first"
    );
    let mut wrong = Vec::new();
    for &kind in ElementKind::ALL {
        let ancestors = model.ancestors(kind.name());
        for &class in AbstractClass::ALL {
            let expected = ancestors.contains(&class.name());
            if kind.is_a(class) != expected {
                wrong.push(format!(
                    "{}.is_a({}) should be {expected}",
                    kind.name(),
                    class.name()
                ));
            }
        }
        for &other in ElementKind::ALL {
            let expected = kind == other || ancestors.contains(&other.name());
            if kind.is_kind_of(other) != expected {
                wrong.push(format!(
                    "{}.is_kind_of({}) should be {expected}",
                    kind.name(),
                    other.name()
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{ROW}: {} of 38 x 13 + 38 x 38 pairs disagree: {wrong:#?}",
        wrong.len()
    );
}

#[test]
fn lib_tool_004_compositions_agree_with_may_contain() {
    let Some(model) = corrected_model() else {
        return;
    };
    let mut wrong = Vec::new();
    let mut held = 0;
    for &parent in ElementKind::ALL {
        let compositions: Vec<&Property> = model
            .properties_of(parent.name())
            .into_iter()
            .map(|(_, p)| p)
            .filter(|p| p.kind == PropertyKind::Composition)
            .collect();
        for &child in ElementKind::ALL {
            let ancestors = model.ancestors(child.name());
            let expected = compositions
                .iter()
                .any(|p| p.type_name == child.name() || ancestors.contains(&p.type_name.as_str()));
            held += usize::from(expected);
            if parent.may_contain(child) != expected {
                wrong.push(format!(
                    "{}.may_contain({}) should be {expected}",
                    parent.name(),
                    child.name()
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{ROW}: {} of 1,444 pairs disagree: {wrong:#?}",
        wrong.len()
    );
    assert_eq!(
        held, 339,
        "{ROW}: 339 of the 1,444 pairs are admitted by a composition"
    );
}

#[test]
fn lib_tool_004_reference_ends_and_bounds_agree_with_reference_end() {
    let Some(model) = corrected_model() else {
        return;
    };
    let mut from_model = BTreeSet::new();
    for &kind in ElementKind::ALL {
        for (_, p) in model.properties_of(kind.name()) {
            if p.kind == PropertyKind::Reference {
                from_model.insert((
                    kind.name().to_owned(),
                    p.name.clone(),
                    to_usize(p.lower),
                    p.upper.map(to_usize),
                ));
            }
        }
    }
    let mut from_crate = BTreeSet::new();
    for &end in ReferenceEnd::ALL
        .iter()
        .filter(|&&e| e != ReferenceEnd::Expression)
    {
        for &kind in ElementKind::ALL {
            if let Some((lower, upper)) = end.bounds(kind) {
                from_crate.insert((kind.name().to_owned(), end.name().to_owned(), lower, upper));
            }
        }
    }
    let missing: Vec<_> = from_model.difference(&from_crate).collect();
    let extra: Vec<_> = from_crate.difference(&from_model).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{ROW}: (kind, end, lower, upper) differ: in the model only {missing:#?}; in ReferenceEnd \
         only {extra:#?}"
    );

    let expression: Vec<&Property> = model
        .class("ExpressionLangString")
        .map(|c| {
            c.properties
                .iter()
                .filter(|p| p.name == "expression")
                .collect()
        })
        .unwrap_or_default();
    let [only] = expression.as_slice() else {
        panic!(
            "{ROW}: ExpressionLangString has exactly one property `expression`, got {expression:?}"
        );
    };
    assert_eq!(
        (only.kind, only.type_name.as_str(), only.lower, only.upper),
        (PropertyKind::Reference, "ExpressionElement", 1, Some(1)),
        "{ROW}: ExpressionLangString.expression is a Reference to ExpressionElement [1..1]"
    );
}

#[test]
fn lib_tool_004_flags_agree_with_the_boolean_attributes() {
    let Some(model) = corrected_model() else {
        return;
    };
    let mut wrong = Vec::new();
    let mut booleans = BTreeSet::new();
    for &kind in ElementKind::ALL {
        let properties = model.properties_of(kind.name());
        for (_, p) in &properties {
            if p.kind == PropertyKind::Data && p.type_name == BOOLEAN {
                booleans.insert(p.name.clone());
            }
        }
        for &flag in Flag::ALL {
            let expected = properties.iter().any(|(_, p)| {
                p.name == flag.name() && p.kind == PropertyKind::Data && p.type_name == BOOLEAN
            });
            if flag.applies_to(kind) != expected {
                wrong.push(format!(
                    "{}.applies_to({}) should be {expected}",
                    flag.name(),
                    kind.name()
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{ROW}: Flag::applies_to disagrees with the model: {wrong:#?}"
    );
    let flags: BTreeSet<String> = Flag::ALL.iter().map(|f| f.name().to_owned()).collect();
    assert_eq!(
        booleans, flags,
        "{ROW}: every Boolean attribute of a kind is a Flag"
    );
}

// ---------------------------------------------------------------------------
// 10: the inventory
// ---------------------------------------------------------------------------

/// One `### Class` section of the inventory.
#[derive(Debug, Default)]
struct Section {
    package: String,
    is_abstract: bool,
    /// The `Generalization chain:` line, split at ` -> `; empty when absent.
    chain: Vec<String>,
    /// The rows of the `Attribute`, `Reference end` and `Containment role`
    /// tables: (name, type, multiplicity up to its first space).
    tables: BTreeMap<String, Vec<(String, String, String)>>,
    /// Every other line.
    text: Vec<String>,
}

/// The inventory as the test reads it.
#[derive(Debug, Default)]
struct Inventory {
    /// Package -> (classes, abstract, enumerations).
    summary: BTreeMap<String, (usize, usize, usize)>,
    /// Class -> its section.
    sections: BTreeMap<String, Section>,
    /// Enumeration -> (package, literals).
    enumerations: BTreeMap<String, (String, Vec<String>)>,
}

/// The cells of a table row `| a | b |`.
fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

/// A count cell.
fn count(cell: &str) -> usize {
    cell.parse()
        .unwrap_or_else(|_| panic!("{ROW}: {INVENTORY}: {cell:?} is no count"))
}

/// Reads one line of a class section into `section`: its generalization
/// chain, a row of the table `table` is in, or prose (which ends a table).
fn read_section_line(section: &mut Section, table: &mut Option<String>, line: &str) {
    if let Some(chain) = line.strip_prefix("Generalization chain: ") {
        section.chain = chain.split(" -> ").map(str::to_owned).collect();
        return;
    }
    if line.starts_with("| ---") {
        return;
    }
    if !line.starts_with('|') {
        *table = None;
        section.text.push(line.to_owned());
        return;
    }
    let row = cells(line);
    let header = ["Attribute", "Reference end", "Containment role"];
    if table.is_none() && row.first().is_some_and(|c| header.contains(&c.as_str())) {
        *table = Some(row[0].clone());
        section.tables.insert(row[0].clone(), Vec::new());
    } else if let (Some(t), [name, ty, multiplicity, ..]) = (table.as_ref(), row.as_slice()) {
        let multiplicity = multiplicity.split(' ').next().unwrap_or_default();
        section.tables.entry(t.clone()).or_default().push((
            name.clone(),
            ty.clone(),
            multiplicity.to_owned(),
        ));
    }
}

/// Reads the inventory's summary, class sections and enumerations.
fn read_inventory(text: &str) -> Inventory {
    enum Part {
        Other,
        Summary,
        Package(String),
        Enumerations,
    }
    let mut inventory = Inventory::default();
    let mut part = Part::Other;
    let mut class: Option<String> = None;
    let mut table: Option<String> = None;
    let mut enumeration: Option<String> = None;
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            class = None;
            table = None;
            part = match heading {
                "Package summary" => Part::Summary,
                "Enumerations" => Part::Enumerations,
                _ => heading
                    .strip_prefix("Package `")
                    .and_then(|rest| rest.strip_suffix('`'))
                    .map_or(Part::Other, |name| Part::Package(name.to_owned())),
            };
            continue;
        }
        match &part {
            Part::Summary if line.starts_with('|') && !line.starts_with("| ---") => {
                let row = cells(line);
                if row.first().is_some_and(|c| c != "Package") && row.len() == 4 {
                    inventory.summary.insert(
                        row[0].clone(),
                        (count(&row[1]), count(&row[2]), count(&row[3])),
                    );
                }
            }
            Part::Package(package) => {
                if let Some(heading) = line.strip_prefix("### ") {
                    let (name, is_abstract) = heading
                        .strip_suffix(" (abstract)")
                        .map_or((heading, false), |name| (name, true));
                    inventory.sections.insert(
                        name.to_owned(),
                        Section {
                            package: package.clone(),
                            is_abstract,
                            ..Section::default()
                        },
                    );
                    class = Some(name.to_owned());
                    table = None;
                    continue;
                }
                if let Some(section) = class.as_ref().and_then(|c| inventory.sections.get_mut(c)) {
                    read_section_line(section, &mut table, line);
                }
            }
            Part::Enumerations => {
                if let Some(heading) = line.strip_prefix("### ") {
                    let (name, package) = heading.split_once(" (`").unwrap_or((heading, ""));
                    let package = package.trim_end_matches("`)");
                    inventory
                        .enumerations
                        .insert(name.to_owned(), (package.to_owned(), Vec::new()));
                    enumeration = Some(name.to_owned());
                } else if let Some(literals) = line.strip_prefix("Literals: ") {
                    if let Some(entry) = enumeration
                        .as_ref()
                        .and_then(|e| inventory.enumerations.get_mut(e))
                    {
                        entry.1 = literals
                            .split(", ")
                            .map(|l| l.trim_matches('`').to_owned())
                            .collect();
                    }
                }
            }
            _ => {}
        }
    }
    inventory
}

/// A multiplicity as the inventory writes it.
fn multiplicity(lower: u32, upper: Option<u32>) -> String {
    match upper {
        Some(upper) if upper == lower => lower.to_string(),
        Some(upper) => format!("{lower}..{upper}"),
        None => format!("{lower}..*"),
    }
}

/// A property's type as the inventory writes it.
fn type_cell(p: &Property) -> String {
    match (p.kind, p.type_name.as_str()) {
        (PropertyKind::Data, BOOLEAN) => "Boolean".to_owned(),
        (_, name) => name.to_owned(),
    }
}

/// The inventory table a property is listed in.
fn table_of(kind: PropertyKind) -> &'static str {
    match kind {
        PropertyKind::Data | PropertyKind::Enumeration => "Attribute",
        PropertyKind::Reference => "Reference end",
        PropertyKind::Composition => "Containment role",
    }
}

/// The differences between the inventory's section of `class` and the model.
fn section_differences(model: &Metamodel, class: &Class, section: &Section) -> Vec<String> {
    let mut wrong = Vec::new();
    let package = model.package_of(&class.name).unwrap_or_default();
    if section.package != package || section.is_abstract != class.is_abstract {
        wrong.push(format!(
            "{}: package {} abstract {} in the inventory, {package} {} in the model",
            class.name, section.package, section.is_abstract, class.is_abstract
        ));
    }
    let chain: Vec<&str> = section.chain.iter().map(String::as_str).collect();
    if chain != model.ancestors(&class.name) {
        wrong.push(format!(
            "{}: generalization chain {chain:?}, model {:?}",
            class.name,
            model.ancestors(&class.name)
        ));
    }
    let mut expected: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    for p in &class.properties {
        if class.name == "Resource" && p.name == "location" {
            continue; // D5: followed from the text, stated in prose (below).
        }
        let cell = multiplicity(p.lower, p.upper);
        assert!(
            ["1", "0..1", "0..*", "1..*", "2..*"].contains(&cell.as_str()),
            "{ROW}: {}.{} has the multiplicity {cell}, which the inventory has no form for",
            class.name,
            p.name
        );
        expected
            .entry(table_of(p.kind).to_owned())
            .or_default()
            .push((p.name.clone(), type_cell(p), cell));
    }
    if expected != section.tables {
        wrong.push(format!(
            "{}: tables {:?}, model {expected:?}",
            class.name, section.tables
        ));
    }
    wrong
}

/// The inventory text.
fn inventory_text() -> String {
    let path = xtask::repo_root().join(INVENTORY);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{ROW}: read {}: {e}", path.display()))
}

/// The inventory's provenance and the stale bullets of the design brief.
fn assert_inventory_wording(text: &str) {
    assert!(
        text.contains(
            "A test (row LIB-TOOL-004) compares this page with the pinned model; edit it only \
             together with that test."
        ) && !text.contains("Until this repository has its own"),
        "{ROW}: {INVENTORY} says that this test compares it with the model"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("- Top interchange objects")
                && l.contains("`TerminologyPackage`")),
        "{ROW}: {INVENTORY} names TerminologyPackage as a top interchange object"
    );
    assert!(
        !text.contains("that is not built yet"),
        "{ROW}: {INVENTORY} no longer says that LIB-COMPAT-004 is not built"
    );
}

#[test]
fn lib_tool_004_the_inventory_agrees_with_the_model() {
    let text = inventory_text();
    assert_inventory_wording(&text);
    let Some(model) = corrected_model() else {
        return;
    };
    let inventory = read_inventory(&text);

    let summary: BTreeMap<String, (usize, usize, usize)> = model
        .packages
        .iter()
        .map(|p| {
            let classes = p.classes.len();
            let abstract_count = p.classes.iter().filter(|c| c.is_abstract).count();
            (
                p.name.clone(),
                (classes, abstract_count, p.enumerations.len()),
            )
        })
        .collect();
    let expected: BTreeMap<String, (usize, usize, usize)> = [
        ("Base", (12, 5, 0)),
        ("AssuranceCase", (3, 0, 0)),
        ("Terminology", (10, 3, 0)),
        ("Argumentation", (16, 4, 1)),
        ("Artifact", (13, 1, 0)),
    ]
    .map(|(name, counts)| (name.to_owned(), counts))
    .into();
    assert_eq!(
        summary, expected,
        "{ROW}: the corrected model's package counts"
    );
    assert_eq!(
        inventory.summary, summary,
        "{ROW}: the Package summary of {INVENTORY}"
    );

    let in_model: BTreeSet<&str> = classes(&model).map(|c| c.name.as_str()).collect();
    let in_inventory: BTreeSet<&str> = inventory.sections.keys().map(String::as_str).collect();
    assert_eq!(
        in_inventory, in_model,
        "{ROW}: {INVENTORY} has one section per class"
    );
    let wrong: Vec<String> = classes(&model)
        .filter_map(|c| {
            inventory
                .sections
                .get(&c.name)
                .map(|s| section_differences(&model, c, s))
        })
        .flatten()
        .collect();
    assert!(
        wrong.is_empty(),
        "{ROW}: {INVENTORY} differs from the model: {wrong:#?}"
    );

    // Resource.location (D5) is in prose, never in a table.
    assert!(
        inventory.sections.values().all(|s| s
            .tables
            .values()
            .flatten()
            .all(|row| row.0 != "location")),
        "{ROW}: Resource.location is in no table of {INVENTORY}"
    );
    assert!(
        inventory
            .sections
            .get("Resource")
            .is_some_and(|s| s.text.iter().any(|l| {
                l.starts_with("Absent from ptc/22-03-13 and followed from the text")
                    && l.contains("location")
            })),
        "{ROW}: the Resource section says that `location` is absent from ptc/22-03-13 and \
         followed from the text"
    );

    let enumerations: BTreeMap<String, (String, Vec<String>)> = model
        .packages
        .iter()
        .flat_map(|p| {
            p.enumerations
                .iter()
                .map(move |e| (e.name.clone(), (p.name.clone(), e.literals.clone())))
        })
        .collect();
    assert_eq!(
        inventory.enumerations, enumerations,
        "{ROW}: the Enumerations of {INVENTORY}"
    );
}

// ---------------------------------------------------------------------------
// 11: what a strict read and write keep
// ---------------------------------------------------------------------------

/// The role of a utility element under a `ModelElement`.
fn utility_role(kind: ElementKind) -> Option<&'static str> {
    match kind {
        ElementKind::Description => Some("description"),
        ElementKind::ImplementationConstraint => Some("implementationConstraint"),
        ElementKind::Note => Some("note"),
        ElementKind::TaggedValue => Some("taggedValue"),
        _ => None,
    }
}

/// The smallest document holding an element `e` of `kind` with `attribute`
/// (`name="value"`): the root if `kind` is a package, else in the root
/// package of its own package, or under an `ArgumentPackage` for a utility
/// element.
fn document_with(kind: ElementKind, attribute: &str) -> String {
    let k = kind.name();
    let name = r#"<name lang="en" content="n"/>"#;
    if kind.is_package() {
        return format!(r#"<sacm:{k} {NS} xmi:id="e" {attribute}>{name}</sacm:{k}>"#);
    }
    if let Some(role) = utility_role(kind) {
        return format!(
            r#"<sacm:ArgumentPackage {NS} xmi:id="p">{name}<{role} xmi:id="e" {attribute}/></sacm:ArgumentPackage>"#
        );
    }
    let (root, role) = match kind.package() {
        MetamodelPackage::Argumentation => ("ArgumentPackage", "argumentElement"),
        MetamodelPackage::Artifact => ("ArtifactPackage", "artifactElement"),
        MetamodelPackage::Terminology => ("TerminologyPackage", "terminologyElement"),
        other => panic!("{ROW}: setup: no package to hold a {k} of {other}"),
    };
    format!(
        r#"<sacm:{root} {NS} xmi:id="p">{name}<{role} xmi:type="sacm:{k}" xmi:id="e" {attribute}>{name}</{role}></sacm:{root}>"#
    )
}

/// The value test 11 gives a Data or Enumeration property.
fn sample_value_of(p: &Property) -> &'static str {
    match (p.kind, p.type_name.as_str()) {
        (PropertyKind::Data, BOOLEAN) => "true",
        (PropertyKind::Data, DATE) => "2026-01-01",
        (PropertyKind::Data, _) => "x",
        (PropertyKind::Enumeration, "AssertionDeclaration") => "needsSupport",
        _ => panic!(
            "{ROW}: setup: no sample value for {} of type {}",
            p.name, p.type_name
        ),
    }
}

/// Reads `document` strictly and writes it strictly; the problems found, if
/// the read keeps preserved content, reports SACM-XMI-003 or the write lacks
/// one of `expected`.
fn strict_round(what: &str, document: &str, expected: &[&str]) -> Vec<String> {
    let output = match xmi::read(
        document.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Strict),
    ) {
        Ok(output) => output,
        Err(error) => {
            return vec![format!(
                "{what}: a strict read refuses it: {error}\n{document}"
            )];
        }
    };
    let mut wrong = Vec::new();
    if output.document().has_preserved_content() {
        wrong.push(format!(
            "{what}: a strict read keeps preserved content\n{document}"
        ));
    }
    let unknown: Vec<String> = output
        .diagnostics()
        .iter()
        .filter(|d| d.code().as_str() == "SACM-XMI-003")
        .map(ToString::to_string)
        .collect();
    if !unknown.is_empty() {
        wrong.push(format!("{what}: a strict read reports {unknown:?}"));
    }
    match xmi::write(output.document(), &WriteOptions::default()) {
        Err(error) => wrong.push(format!("{what}: a strict write refuses it: {error}")),
        Ok(written) => {
            for text in expected {
                if !written.contains(text) {
                    wrong.push(format!(
                        "{what}: the strict write lacks {text:?}:\n{written}"
                    ));
                }
            }
        }
    }
    wrong
}

/// The value compositions, the `LangString` attributes and the
/// `ExpressionLangString` reference, each with a document using it and what
/// its strict write must contain.
fn value_documents() -> Vec<((&'static str, &'static str), String, Vec<&'static str>)> {
    let argument = |body: &str| {
        format!(
            r#"<sacm:ArgumentPackage {NS} xmi:id="p"><name lang="sv" content="n"/>{body}</sacm:ArgumentPackage>"#
        )
    };
    let content = argument(
        r#"<description xmi:id="d"><content><value lang="en" content="c"/></content></description>"#,
    );
    let key = argument(
        r#"<taggedValue xmi:id="t"><key><value lang="en" content="k"/></key></taggedValue>"#,
    );
    let location = format!(
        r#"<sacm:ArtifactPackage {NS} xmi:id="p"><name lang="en" content="n"/><artifactElement xmi:type="sacm:Resource" xmi:id="r"><name lang="en" content="n"/><location><value lang="en" content="l"/></location></artifactElement></sacm:ArtifactPackage>"#
    );
    let expression = format!(
        r#"<sacm:TerminologyPackage {NS} xmi:id="p"><name lang="en" content="n"/><terminologyElement xmi:type="sacm:Term" xmi:id="t1" value="v"><name lang="en" content="t"/></terminologyElement><terminologyElement xmi:type="sacm:Term" xmi:id="t2" value="w"><name xmi:type="sacm:ExpressionLangString" lang="en" expression="t1"/></terminologyElement></sacm:TerminologyPackage>"#
    );
    vec![
        (
            ("ModelElement", "name"),
            argument(""),
            vec!["<name ", r#"content="n""#],
        ),
        (
            ("UtilityElement", "content"),
            content.clone(),
            vec!["<content>", r#"content="c""#],
        ),
        (("TaggedValue", "key"), key, vec!["<key>", r#"content="k""#]),
        (
            ("Resource", "location"),
            location,
            vec!["<location", r#"content="l""#],
        ),
        (
            ("MultiLangString", "value"),
            content.clone(),
            vec!["<value ", r#"content="c""#],
        ),
        (("LangString", "lang"), argument(""), vec![r#"lang="sv""#]),
        (("LangString", "content"), content, vec![r#"content="c""#]),
        (
            ("ExpressionLangString", "expression"),
            expression,
            vec![r#"expression="t1""#],
        ),
    ]
}

#[test]
fn lib_tool_004_every_attribute_and_value_composition_is_read_and_written() {
    let Some(model) = corrected_model() else {
        return;
    };
    let mut wrong = Vec::new();
    let mut checked = 0;
    for &kind in ElementKind::ALL {
        for (_, p) in model.properties_of(kind.name()) {
            if matches!(p.kind, PropertyKind::Data | PropertyKind::Enumeration) {
                let attribute = format!(r#"{}="{}""#, p.name, sample_value_of(p));
                let what = format!("{} {attribute}", kind.name());
                wrong.extend(strict_round(
                    &what,
                    &document_with(kind, &attribute),
                    &[&format!(" {attribute}")],
                ));
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "{ROW}: setup: no attribute was checked");

    // The value parts: every composition typed by a value type, and the
    // properties of the value types themselves, as the model declares them.
    let mut value_parts = BTreeSet::new();
    for c in classes(&model) {
        for p in &c.properties {
            let of_value_type = VALUE_TYPES.contains(&c.name.as_str());
            let typed_by_value =
                p.kind == PropertyKind::Composition && VALUE_TYPES.contains(&p.type_name.as_str());
            if typed_by_value || (of_value_type && p.kind != PropertyKind::Composition) {
                value_parts.insert((c.name.clone(), p.name.clone()));
            }
        }
    }
    let documents = value_documents();
    let listed: BTreeSet<(String, String)> = documents
        .iter()
        .map(|((c, p), _, _)| ((*c).to_owned(), (*p).to_owned()))
        .collect();
    assert_eq!(
        listed, value_parts,
        "{ROW}: the value parts of the model are the ones checked here"
    );
    for ((class, property), document, expected) in &documents {
        wrong.extend(strict_round(
            &format!("{class}.{property}"),
            document,
            expected,
        ));
    }
    assert!(
        wrong.is_empty(),
        "{ROW}: {} problems:\n{}",
        wrong.len(),
        wrong.join("\n")
    );

    // Control: an attribute no class has is preserved content, an Error, and
    // a strict write refuses it.
    let bogus = document_with(ElementKind::Claim, r#"bogus="x""#);
    let output = xmi::read(
        bogus.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Strict),
    )
    .unwrap_or_else(|e| panic!("{ROW}: the control document reads strictly: {e}"));
    assert!(
        output.document().has_preserved_content(),
        "{ROW}: bogus=\"x\" on a Claim is preserved content"
    );
    assert!(
        output
            .diagnostics()
            .iter()
            .any(|d| d.code().as_str() == "SACM-XMI-003"
                && d.severity() == Severity::Error
                && d.affected().iter().map(ElementId::as_str).eq(["e"])),
        "{ROW}: bogus=\"x\" on a Claim is one SACM-XMI-003 Error affecting e, got {:?}",
        output.diagnostics()
    );
    assert!(
        matches!(
            xmi::write(output.document(), &WriteOptions::default()),
            Err(WriteError::PreservedContent { .. })
        ),
        "{ROW}: a strict write refuses the control document"
    );
}
