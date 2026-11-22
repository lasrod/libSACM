//! The SACM 2.3 metamodel as OMG's machine-readable model, ptc/22-03-13,
//! declares it (LIB-TOOL-004), and the five recorded corrections that make it
//! say what the specification text says.
//!
//! [`parse`] reads a UML model in the shape of ptc/22-03-13: one `uml:Model`
//! holding packages, each holding classes, enumerations and associations.
//! [`load`] reads the pinned file that `cargo xtask fetch-spec` puts in
//! `third_party/sacm-2.3/` and checks its SHA-256 first.
//! [`Metamodel::corrected`] applies [`CORRECTIONS`].
//!
//! The tests in `xtask/tests/metamodel.rs` compare the corrected model with
//! the crate's metadata and with `docs/sacm/metamodel-inventory.md`, and
//! [`crate::schema`] generates the SACM XML Schema from it.
//!
//! ```
//! use xtask::metamodel::{PropertyKind, parse};
//!
//! let model = parse(r#"<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
//!   <uml:Model xmi:type="uml:Model" xmi:id="m" name="Model">
//!     <packagedElement xmi:type="uml:Package" xmi:id="p" name="Sample">
//!       <packagedElement xmi:type="uml:Class" xmi:id="Tag" name="Tag"/>
//!       <packagedElement xmi:type="uml:Class" xmi:id="Box" name="Box">
//!         <ownedAttribute xmi:type="uml:Property" xmi:id="Box.tag" name="tag" type="Tag" aggregation="composite"/>
//!       </packagedElement>
//!     </packagedElement>
//!   </uml:Model>
//! </xmi:XMI>"#)?;
//! let (declaring, tag) = model.properties_of("Box")[0];
//! assert_eq!((declaring, tag.name.as_str(), tag.kind), ("Box", "tag", PropertyKind::Composition));
//! assert_eq!((tag.lower, tag.upper), (1, Some(1)));
//! # Ok::<(), String>(())
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use roxmltree::{Document, Node};

use crate::fetch::{ARTIFACTS, sha256_hex};

/// The file name of the pinned ptc/22-03-13 in the directory that
/// `cargo xtask fetch-spec` fills.
pub const MODEL_FILE: &str = "SACM-2.3-ptc-22-03-13.xml";

/// The XMI 2.5.1 namespace, which ptc/22-03-13 uses for `xmi:id`,
/// `xmi:idref` and `xmi:type`.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The UML 2.5 namespace of ptc/22-03-13.
const UML_NS: &str = "http://www.omg.org/spec/UML/20131001";

/// The fix named in every refusal of a missing or changed reference.
const FETCH: &str = "run `cargo xtask fetch-spec`";

/// A metamodel: its packages in document order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metamodel {
    /// The packages, in document order.
    pub packages: Vec<Package>,
}

/// One package of the metamodel (`Base`, `AssuranceCase`, `Artifact`,
/// `Argumentation`, `Terminology` for SACM 2.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    /// The package's name.
    pub name: String,
    /// Its classes, in document order.
    pub classes: Vec<Class>,
    /// Its enumerations, in document order.
    pub enumerations: Vec<Enumeration>,
}

/// One class of the metamodel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Class {
    /// The class's name.
    pub name: String,
    /// Whether the class is abstract.
    pub is_abstract: bool,
    /// The name of the class it specializes, if any (ptc/22-03-13 has single
    /// inheritance only).
    pub superclass: Option<String>,
    /// The properties the class itself owns, in document order.
    pub properties: Vec<Property>,
}

/// One property a class owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    /// The property's name; `""` when the model gives none (spec defect D8,
    /// which correction `D8` repairs).
    pub name: String,
    /// The name of the Association the property is an end of, `""` for an
    /// unnamed one; `None` when it is an end of none.
    pub association: Option<String>,
    /// How the property is typed.
    pub kind: PropertyKind,
    /// The name of the class or enumeration that types it; for
    /// [`PropertyKind::Data`], the fragment of the `<type href>` (`String`,
    /// or a `MagicDraw` id such as `eee_1045467100323_191782_59`).
    pub type_name: String,
    /// The lower bound of the multiplicity.
    pub lower: u32,
    /// The upper bound of the multiplicity; `None` for `*`.
    pub upper: Option<u32>,
}

/// How a property is typed, which decides how XMI serializes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyKind {
    /// Typed by a type outside the model (`<type href>`): a UML primitive or
    /// a `MagicDraw` type.
    Data,
    /// Typed by an enumeration of the model.
    Enumeration,
    /// Typed by a class of the model, not composite.
    Reference,
    /// Typed by a class of the model, with `aggregation="composite"`.
    Composition,
}

/// One enumeration of the metamodel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enumeration {
    /// The enumeration's name.
    pub name: String,
    /// Its literals, in model order.
    pub literals: Vec<String>,
}

/// One recorded correction of ptc/22-03-13, where the model and the
/// specification text disagree and the text is followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Correction {
    /// The spec defect (`D8`) or clause (`§11.3`) that records it.
    pub id: &'static str,
    /// What it changes.
    pub summary: &'static str,
}

/// The corrections [`Metamodel::corrected`] applies, in this order (ADR
/// 0043 S3; `docs/sacm/reference/specification-defects.md`).
pub const CORRECTIONS: &[Correction] = &[
    Correction {
        id: "D8",
        summary: "a property with no name takes the name of its Association",
    },
    Correction {
        id: "D19",
        summary: "class ArtifactAssertedRelationship is named ArtifactAssetRelationship and \
                  specializes ArtifactAsset",
    },
    Correction {
        id: "D17",
        summary: "Event.occurece is named date",
    },
    Correction {
        id: "D5",
        summary: "Resource has the composition location: MultiLangString [0..1]",
    },
    Correction {
        id: "§11.3",
        summary: "ArgumentationElement is abstract",
    },
];

impl Metamodel {
    /// This model with [`CORRECTIONS`] applied, in order.
    ///
    /// # Errors
    ///
    /// Returns a message naming the correction whose subject is absent: no
    /// unnamed property (D8), or one whose Association is unnamed too; no
    /// class `ArtifactAssertedRelationship` or `ArtifactAsset` (D19); no
    /// `Event.occurece` (D17); no class `Resource`, or one that already has
    /// `location` (D5); no class `ArgumentationElement` (§11.3). A corrected
    /// model therefore cannot be corrected again.
    pub fn corrected(&self) -> Result<Metamodel, String> {
        let mut model = self.clone();
        for correction in CORRECTIONS {
            apply(&mut model, correction.id).map_err(|what| {
                format!(
                    "correction {} ({}): {what}",
                    correction.id, correction.summary
                )
            })?;
        }
        Ok(model)
    }

    /// The class named `name`, if the model has one.
    #[must_use]
    pub fn class(&self, name: &str) -> Option<&Class> {
        self.classes().find(|c| c.name == name)
    }

    /// The name of the package that holds the class `class`.
    #[must_use]
    pub fn package_of(&self, class: &str) -> Option<&str> {
        self.packages
            .iter()
            .find(|p| p.classes.iter().any(|c| c.name == class))
            .map(|p| p.name.as_str())
    }

    /// The superclasses of `class`, nearest first. The walk stops at a
    /// superclass the model does not hold and before a class seen already.
    #[must_use]
    pub fn ancestors(&self, class: &str) -> Vec<&str> {
        let mut ancestors: Vec<&str> = Vec::new();
        let mut current = self.class(class);
        while let Some(superclass) = current.and_then(|c| c.superclass.as_deref()) {
            if superclass == class || ancestors.contains(&superclass) {
                break;
            }
            ancestors.push(superclass);
            current = self.class(superclass);
        }
        ancestors
    }

    /// Every property of `class`, inherited ones first: the root-most
    /// superclass's first, then each subclass's, each in model order. Each
    /// comes with the name of the class that declares it. Empty for a class
    /// the model does not hold.
    #[must_use]
    pub fn properties_of(&self, class: &str) -> Vec<(&str, &Property)> {
        let Some(own) = self.class(class) else {
            return Vec::new();
        };
        let mut chain: Vec<&Class> = self
            .ancestors(class)
            .into_iter()
            .filter_map(|name| self.class(name))
            .collect();
        chain.reverse();
        chain.push(own);
        chain
            .into_iter()
            .flat_map(|c| c.properties.iter().map(move |p| (c.name.as_str(), p)))
            .collect()
    }

    /// XMI 2.5.1 rule 4f's candidate list for a property typed `class`:
    /// `class` if it is concrete, then the candidates of each immediate
    /// subclass, the subclasses in byte order of name ("alphabetical order of
    /// immediate subclasses with depth first expansion").
    #[must_use]
    pub fn candidates(&self, class: &str) -> Vec<&str> {
        let mut seen = BTreeSet::new();
        let mut candidates = Vec::new();
        self.collect_candidates(class, &mut seen, &mut candidates);
        candidates
    }

    fn collect_candidates<'a>(
        &'a self,
        class: &str,
        seen: &mut BTreeSet<&'a str>,
        candidates: &mut Vec<&'a str>,
    ) {
        let Some(own) = self.class(class) else {
            return;
        };
        if !seen.insert(own.name.as_str()) {
            return;
        }
        if !own.is_abstract {
            candidates.push(own.name.as_str());
        }
        let mut subclasses: Vec<&str> = self
            .classes()
            .filter(|c| c.superclass.as_deref() == Some(class))
            .map(|c| c.name.as_str())
            .collect();
        subclasses.sort_unstable();
        for subclass in subclasses {
            self.collect_candidates(subclass, seen, candidates);
        }
    }

    /// Every class, in document order.
    fn classes(&self) -> impl Iterator<Item = &Class> {
        self.packages.iter().flat_map(|p| p.classes.iter())
    }
}

/// Applies the correction `id` to `model`, or says which subject is absent.
fn apply(model: &mut Metamodel, id: &str) -> Result<(), String> {
    match id {
        "D8" => {
            let mut renamed = 0_usize;
            for class in model.packages.iter_mut().flat_map(|p| p.classes.iter_mut()) {
                for property in class.properties.iter_mut().filter(|p| p.name.is_empty()) {
                    match property.association.as_deref() {
                        Some(association) if !association.is_empty() => {
                            association.clone_into(&mut property.name);
                            renamed += 1;
                        }
                        _ => {
                            return Err(format!(
                                "an unnamed property of {} has no named Association",
                                class.name
                            ));
                        }
                    }
                }
            }
            if renamed == 0 {
                return Err("no unnamed property is present".to_owned());
            }
            Ok(())
        }
        "D19" => {
            if model.class("ArtifactAsset").is_none() {
                return Err("class ArtifactAsset is absent".to_owned());
            }
            let class = class_mut(model, "ArtifactAssertedRelationship")?;
            "ArtifactAssetRelationship".clone_into(&mut class.name);
            class.superclass = Some("ArtifactAsset".to_owned());
            Ok(())
        }
        "D17" => {
            let event = class_mut(model, "Event")?;
            let property = event
                .properties
                .iter_mut()
                .find(|p| p.name == "occurece")
                .ok_or_else(|| "property Event.occurece is absent".to_owned())?;
            "date".clone_into(&mut property.name);
            Ok(())
        }
        "D5" => {
            let resource = class_mut(model, "Resource")?;
            if resource.properties.iter().any(|p| p.name == "location") {
                return Err("Resource already has location".to_owned());
            }
            resource.properties.push(Property {
                name: "location".to_owned(),
                association: None,
                kind: PropertyKind::Composition,
                type_name: "MultiLangString".to_owned(),
                lower: 0,
                upper: Some(1),
            });
            Ok(())
        }
        "§11.3" => {
            class_mut(model, "ArgumentationElement")?.is_abstract = true;
            Ok(())
        }
        other => Err(format!("{other} is no known correction")),
    }
}

/// The class `name` of `model`, mutable, or the message that it is absent.
fn class_mut<'a>(model: &'a mut Metamodel, name: &str) -> Result<&'a mut Class, String> {
    model
        .packages
        .iter_mut()
        .flat_map(|p| p.classes.iter_mut())
        .find(|c| c.name == name)
        .ok_or_else(|| format!("class {name} is absent"))
}

/// Reads the pinned [`MODEL_FILE`] in `spec_dir`, checks its SHA-256
/// against `xtask::fetch::ARTIFACTS` and parses it, as written: apply
/// [`Metamodel::corrected`] for the corrections.
///
/// # Errors
///
/// Returns a message naming the file and `cargo xtask fetch-spec` when it is
/// missing or has another SHA-256 (with both hashes); otherwise a read error,
/// a file that is not UTF-8, or the refusal of [`parse`], each naming the
/// file.
pub fn load(spec_dir: &Path) -> Result<Metamodel, String> {
    let path = spec_dir.join(MODEL_FILE);
    let shown = path.display().to_string();
    let bytes = read_pinned(&path, &shown, MODEL_FILE)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("{shown} is not UTF-8: {e}"))?;
    parse(text).map_err(|e| format!("{shown}: {e}"))
}

/// The bytes of the reference `file_name` at `path`, shown as `shown` in
/// messages, after checking them against their pin in
/// `xtask::fetch::ARTIFACTS`.
pub(crate) fn read_pinned(path: &Path, shown: &str, file_name: &str) -> Result<Vec<u8>, String> {
    let pin = ARTIFACTS
        .iter()
        .find(|a| a.file_name == file_name)
        .map(|a| a.sha256)
        .ok_or_else(|| format!("{file_name} is not in xtask::fetch::ARTIFACTS"))?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(format!("{shown} is missing; {FETCH}"));
        }
        Err(e) => return Err(format!("read {shown}: {e}")),
    };
    let actual = sha256_hex(&bytes);
    if actual != pin {
        return Err(format!(
            "{shown} has SHA-256 {actual}, expected {pin}; {FETCH}"
        ));
    }
    Ok(bytes)
}

/// Parses a UML model in the shape of ptc/22-03-13, as written: packages,
/// classes, properties and enumerations in document order, multiplicities as
/// their `lowerValue` and `upperValue` say (no `lowerValue` is 1, one without
/// a `value` is 0; no `upperValue` is 1, `value="*"` is unbounded).
///
/// Only what the metamodel needs is read: comments, default values and the
/// Association-owned ends are not. A property's [`Property::association`]
/// comes from its `association` attribute or from the `memberEnd` of an
/// Association.
///
/// # Errors
///
/// Returns a message when the text is not well-formed XML or holds no
/// `uml:Model`, when a package is nested in another, when a `packagedElement`
/// of a package is no Class, Enumeration or Association, when a class has two
/// generalizations, when an idref (a type, a general, an association or a
/// member end) names no element, when two classes have the same name, or when
/// a name, a type or a bound is missing or malformed.
pub fn parse(xml: &str) -> Result<Metamodel, String> {
    let document =
        Document::parse(xml).map_err(|e| format!("the model is not well-formed XML: {e}"))?;
    let root = document.root_element();
    let model = if is_uml_model(root) {
        root
    } else {
        root.children()
            .find(|n| is_uml_model(*n))
            .ok_or_else(|| "the model holds no uml:Model element".to_owned())?
    };

    let mut ids = Ids(BTreeMap::new());
    for node in document.descendants() {
        if let Some(id) = node.attribute((XMI_NS, "id")) {
            if ids.0.insert(id, node).is_some() {
                return Err(format!("the xmi:id {id} is declared twice"));
            }
        }
    }

    let packages = package_nodes(model)?;

    // The Association each member end belongs to.
    let mut ends: BTreeMap<&str, &str> = BTreeMap::new();
    for (_, elements) in &packages {
        for &element in elements {
            if uml_type(element) != Some("Association") {
                continue;
            }
            let name = attr(element, "name").unwrap_or("");
            for end in element.children().filter(|n| is_child(n, "memberEnd")) {
                let idref = end.attribute((XMI_NS, "idref")).ok_or_else(|| {
                    format!("a memberEnd of the Association {name:?} has no xmi:idref")
                })?;
                ids.resolve(idref, || format!("a memberEnd of the Association {name:?}"))?;
                ends.insert(idref, name);
            }
        }
    }

    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(packages.len());
    for (name, elements) in packages {
        let mut package = Package {
            name: name.to_owned(),
            classes: Vec::new(),
            enumerations: Vec::new(),
        };
        for element in elements {
            match uml_type(element) {
                Some("Class") => {
                    let class = read_class(element, &ids, &ends)?;
                    if !seen.insert(class.name.clone()) {
                        return Err(format!("the class name {} is declared twice", class.name));
                    }
                    package.classes.push(class);
                }
                Some("Enumeration") => package.enumerations.push(read_enumeration(element)?),
                _ => {}
            }
        }
        result.push(package);
    }
    Ok(Metamodel { packages: result })
}

/// The elements of a model by `xmi:id`.
struct Ids<'a, 'input>(BTreeMap<&'a str, Node<'a, 'input>>);

impl<'a, 'input> Ids<'a, 'input> {
    /// The element `idref` names, or an error naming it and `what` refers to
    /// it.
    fn resolve(
        &self,
        idref: &str,
        what: impl FnOnce() -> String,
    ) -> Result<Node<'a, 'input>, String> {
        self.0
            .get(idref)
            .copied()
            .ok_or_else(|| format!("unresolved idref {idref} ({})", what()))
    }
}

/// The packages of `model` with their `packagedElement`s, after checking
/// that no package is nested and that each element is a Class, an
/// Enumeration or an Association.
fn package_nodes<'a, 'input>(
    model: Node<'a, 'input>,
) -> Result<Vec<(&'a str, Vec<Node<'a, 'input>>)>, String> {
    let mut packages = Vec::new();
    for package in model.children().filter(|n| is_child(n, "packagedElement")) {
        let name = attr(package, "name").unwrap_or("");
        if uml_type(package) != Some("Package") {
            return Err(format!(
                "the packagedElement {name:?} of the uml:Model is a {}, not a uml:Package",
                raw_type(package)
            ));
        }
        let mut elements = Vec::new();
        for element in package
            .children()
            .filter(|n| is_child(n, "packagedElement"))
        {
            let element_name = attr(element, "name").unwrap_or("");
            match uml_type(element) {
                Some("Class" | "Enumeration" | "Association") => elements.push(element),
                Some("Package") => {
                    return Err(format!(
                        "the package {element_name} is nested in the package {name}; packages \
                         are not nested"
                    ));
                }
                _ => {
                    return Err(format!(
                        "the packagedElement {element_name:?} of the package {name} is a {}, not \
                         a uml:Class, uml:Enumeration or uml:Association",
                        raw_type(element)
                    ));
                }
            }
        }
        packages.push((name, elements));
    }
    Ok(packages)
}

/// Reads one `uml:Class`.
fn read_class<'a, 'input>(
    element: Node<'a, 'input>,
    ids: &Ids<'a, 'input>,
    ends: &BTreeMap<&str, &str>,
) -> Result<Class, String> {
    let name = required_name(element, "a uml:Class")?;
    let generalizations: Vec<Node<'_, '_>> = element
        .children()
        .filter(|n| is_child(n, "generalization"))
        .collect();
    let superclass = match generalizations.as_slice() {
        [] => None,
        [generalization] => {
            let general = attr(*generalization, "general")
                .ok_or_else(|| format!("the generalization of {name} names no general"))?;
            let target = ids.resolve(general, || format!("the general of {name}"))?;
            if uml_type(target) != Some("Class") {
                return Err(format!(
                    "the general of {name} is a {}, not a uml:Class",
                    raw_type(target)
                ));
            }
            Some(required_name(target, "the general of a class")?.to_owned())
        }
        more => {
            return Err(format!(
                "the class {name} has {} generalizations; one is supported",
                more.len()
            ));
        }
    };
    let mut properties = Vec::new();
    for attribute in element.children().filter(|n| is_child(n, "ownedAttribute")) {
        properties.push(read_property(name, attribute, ids, ends)?);
    }
    Ok(Class {
        name: name.to_owned(),
        is_abstract: attr(element, "isAbstract") == Some("true"),
        superclass,
        properties,
    })
}

/// Reads one `ownedAttribute` of the class `class`.
fn read_property<'a, 'input>(
    class: &str,
    attribute: Node<'a, 'input>,
    ids: &Ids<'a, 'input>,
    ends: &BTreeMap<&str, &str>,
) -> Result<Property, String> {
    let name = attr(attribute, "name").unwrap_or("");
    let label = if name.is_empty() {
        format!("an unnamed property of {class}")
    } else {
        format!("{class}.{name}")
    };

    let association = match attr(attribute, "association") {
        Some(idref) => {
            let target = ids.resolve(idref, || format!("the association of {label}"))?;
            if uml_type(target) != Some("Association") {
                return Err(format!(
                    "the association of {label} is a {}, not a uml:Association",
                    raw_type(target)
                ));
            }
            Some(attr(target, "name").unwrap_or("").to_owned())
        }
        None => attribute
            .attribute((XMI_NS, "id"))
            .and_then(|id| ends.get(id))
            .map(|&association| association.to_owned()),
    };

    let (kind, type_name) = if let Some(idref) = attr(attribute, "type") {
        let target = ids.resolve(idref, || format!("the type of {label}"))?;
        let kind = match uml_type(target) {
            Some("Class") if attr(attribute, "aggregation") == Some("composite") => {
                PropertyKind::Composition
            }
            Some("Class") => PropertyKind::Reference,
            Some("Enumeration") => PropertyKind::Enumeration,
            _ => {
                return Err(format!(
                    "the type of {label} is a {}, not a uml:Class or uml:Enumeration",
                    raw_type(target)
                ));
            }
        };
        (
            kind,
            required_name(target, &format!("the type of {label}"))?.to_owned(),
        )
    } else {
        let href = attribute
            .children()
            .find(|n| is_child(n, "type"))
            .and_then(|t| attr(t, "href"))
            .ok_or_else(|| format!("{label} has no type"))?;
        let fragment = href.rsplit_once('#').map_or(href, |(_, fragment)| fragment);
        (PropertyKind::Data, fragment.to_owned())
    };

    let lower = match bound(attribute, "lowerValue") {
        Bound::Absent => 1,
        Bound::NoValue => 0,
        Bound::Value(value) => value
            .parse()
            .map_err(|_| format!("the lower bound {value:?} of {label} is not a number"))?,
    };
    let upper = match bound(attribute, "upperValue") {
        Bound::Absent => Some(1),
        Bound::NoValue => return Err(format!("the upperValue of {label} has no value")),
        Bound::Value("*") => None,
        Bound::Value(value) => Some(
            value
                .parse()
                .map_err(|_| format!("the upper bound {value:?} of {label} is not a number"))?,
        ),
    };

    Ok(Property {
        name: name.to_owned(),
        association,
        kind,
        type_name,
        lower,
        upper,
    })
}

/// A `lowerValue` or `upperValue` of a property, as written.
enum Bound<'a> {
    /// The property has none.
    Absent,
    /// It has one without a `value`.
    NoValue,
    /// It has one with this `value`.
    Value(&'a str),
}

/// The bound `tag` of a property.
fn bound<'a>(attribute: Node<'a, '_>, tag: &str) -> Bound<'a> {
    match attribute.children().find(|n| is_child(n, tag)) {
        None => Bound::Absent,
        Some(n) => attr(n, "value").map_or(Bound::NoValue, Bound::Value),
    }
}

/// Reads one `uml:Enumeration`.
fn read_enumeration(element: Node<'_, '_>) -> Result<Enumeration, String> {
    let name = required_name(element, "a uml:Enumeration")?;
    let literals = element
        .children()
        .filter(|n| is_child(n, "ownedLiteral"))
        .map(|l| required_name(l, &format!("a literal of {name}")).map(str::to_owned))
        .collect::<Result<_, _>>()?;
    Ok(Enumeration {
        name: name.to_owned(),
        literals,
    })
}

/// The `name` of `node`, which must have a non-empty one.
fn required_name<'a>(node: Node<'a, '_>, what: &str) -> Result<&'a str, String> {
    attr(node, "name").filter(|n| !n.is_empty()).ok_or_else(|| {
        let id = node.attribute((XMI_NS, "id")).unwrap_or("?");
        format!("{what} ({id}) has no name")
    })
}

/// Whether `node` is the `uml:Model` element.
fn is_uml_model(node: Node<'_, '_>) -> bool {
    node.is_element()
        && node.tag_name().namespace() == Some(UML_NS)
        && node.tag_name().name() == "Model"
}

/// The local name of `node`'s `xmi:type` when its prefix is bound to the
/// UML namespace (`Class` for `uml:Class`).
fn uml_type<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    let (prefix, local) = node.attribute((XMI_NS, "type"))?.split_once(':')?;
    (node.lookup_namespace_uri(Some(prefix)) == Some(UML_NS)).then_some(local)
}

/// `node`'s `xmi:type` as written, for a message.
fn raw_type(node: Node<'_, '_>) -> String {
    node.attribute((XMI_NS, "type")).map_or_else(
        || format!("<{}> without xmi:type", node.tag_name().name()),
        str::to_owned,
    )
}

/// The value of `node`'s attribute `name` in no namespace (`attribute("x")`
/// alone would also match `xmi:x`).
fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.namespace().is_none() && a.name() == name)
        .map(|a| a.value())
}

/// Whether `node` is an element `name` in no namespace.
fn is_child(node: &Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().namespace().is_none() && node.tag_name().name() == name
}
