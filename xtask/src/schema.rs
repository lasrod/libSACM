//! The SACM XML Schema this project generates from ptc/22-03-13, and the
//! check of strict output against it (LIB-TOOL-023, ADR 0043).
//!
//! OMG publishes no XML Schema for SACM 2.3, though its clause 2 defines
//! every compliance point by one. [`generate`] applies the XMI 2.5.1 clause 8
//! schema production rules, with the Table 7.1 default tag values, to a
//! [`Metamodel`], in the pinned namespace, with two departures (ADR 0043):
//! rule 4f's candidate list is replaced by a *candidate type* (one type
//! holding the attributes and children of every candidate, spec defect D30),
//! and rule 7 is not applied (D32). [`cmd_schema`] writes it from the pinned,
//! corrected model, with a copy of the pinned XMI.xsd beside it. It is never
//! committed or distributed: only its SHA-256, [`GENERATED_SHA256`], is.
//!
//! [`cmd_validate`] then runs `xmllint --schema` on every strict golden file
//! and on every strict output in `<DIR>/outputs`, which CI's `SACM XML Schema`
//! job writes with `sacm roundtrip`. [`judge`] reads one xmllint run, and
//! [`validate_with`] takes the lint as a parameter, so both can be exercised
//! without xmllint.
//!
//! ```
//! use xtask::metamodel::parse;
//! use xtask::schema::generate;
//!
//! let model = parse(r#"<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
//!   <uml:Model xmi:type="uml:Model" xmi:id="m" name="Model">
//!     <packagedElement xmi:type="uml:Package" xmi:id="p" name="Sample">
//!       <packagedElement xmi:type="uml:Class" xmi:id="Tag" name="Tag"/>
//!     </packagedElement>
//!   </uml:Model>
//! </xmi:XMI>"#)?;
//! let schema = generate(&model)?;
//! assert!(schema.contains(r#"  <xsd:element name="Tag" type="sacm:Tag"/>"#));
//! # Ok::<(), String>(())
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::fetch::DEFAULT_DEST;
use crate::metamodel::{self, MODEL_FILE, Metamodel, Property, PropertyKind};
use crate::output_error;

/// Where `cargo xtask schema` writes, relative to the repository root. It is
/// under the git-ignored `target/`, so the schema is never committed.
pub const DEFAULT_OUT: &str = "target/schema";

/// The file name of the generated schema in the out directory.
pub const SCHEMA_FILE: &str = "SACM-2.3.xsd";

/// The file name of the copy of the pinned XMI.xsd in the out directory,
/// which [`SCHEMA_FILE`] imports.
pub const XMI_SCHEMA_FILE: &str = "XMI.xsd";

/// The directory, under the out directory, of the strict outputs that
/// [`cmd_validate`] checks.
pub const OUTPUTS_DIR: &str = "outputs";

/// The strict golden files [`cmd_validate`] checks, relative to the
/// repository root: every `*.golden.sacm.xmi` in it.
pub const GOLDEN_DIR: &str = "crates/sacm/tests/data/sacm23/xmi/golden";

/// The SHA-256 of the schema [`generate`] gives for the pinned ptc/22-03-13
/// with [`metamodel::CORRECTIONS`] applied.
pub const GENERATED_SHA256: &str =
    "2dde06a5d97d8c855873092b5a0d37bb8be79ea738412673d55fce56637792cc";

/// The fixtures whose XMI metadata is not valid against XMI.xsd and is
/// written back as read (ADR 0043 S7). A file whose name is one of these, or
/// ends with `__` and one of these (its strict output), may fail validation,
/// but only on XMI-namespace elements; see [`judge`].
pub const METADATA_ONLY: &[&str] = &[
    "xmi-metadata-valid.sacm.xmi",
    "xmi-metadata.golden.sacm.xmi",
];

/// The pinned XMI.xsd, as `cargo xtask fetch-spec` names it.
const XMI_XSD_INPUT: &str = "XMI-2.5.1-XMI.xsd";

/// The target namespace, pinned (D-NS-PIN): the model gives no URI.
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";

/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The line every generated schema has after its XML declaration.
const COMMENT: &str = "<!-- The SACM 2.3 XML Schema as libSACM generates it (cargo xtask schema) \
                       from OMG ptc/22-03-13 by the XMI 2.5.1 clause 8 rules, with the changes \
                       listed in docs/sacm/compliance-points.md. Not an OMG document. -->";

/// What an xmllint error line on an XMI-namespace element contains.
const XMI_ELEMENT_ERROR: &str = "Element '{http://www.omg.org/spec/XMI/20131001}";

/// What every xmllint error line about the document's validity contains.
const VALIDITY_ERROR: &str = "Schemas validity error";

/// The XML Schema of `model`, by the XMI 2.5.1 clause 8 rules with the
/// default tag values, as one UTF-8 document with LF line ends.
///
/// The model is used as given: pass [`Metamodel::corrected`] for the SACM
/// schema. In it:
///
/// - packages come in byte order of name; in each, every class (abstract ones
///   too) as its `complexType` and its global element, by name, then the
///   enumerations by name; after all packages, the candidate types by name
///   (rule 2);
/// - a class type is a repeated choice of its properties' elements,
///   inherited ones first: Data and Enumeration properties (4d), References
///   (4e), Compositions (4f), then `xmi:Extension` and `xmi:extension` (4c);
///   then `xmi:id`, `xmi:ObjectAttribs` and an attribute for each Reference
///   (IDREF or IDREFS, 4i) and each single-valued Data or Enumeration
///   property (4j, 4k);
/// - a Composition is typed by its one candidate, or by the candidate type
///   `<Type>.candidates`, which declares the properties of every candidate,
///   each name once.
///
/// # Errors
///
/// Returns a message starting `schema: ` when two classes have one name, a
/// superclass or a composition's type is no class of the model, a class is
/// its own ancestor, a name is not an XML name, or the properties of one
/// name in a type disagree in kind other than as a Composition and a
/// Reference: `schema: property <name> of <type> is declared as <kind> and
/// <kind>`.
pub fn generate(model: &Metamodel) -> Result<String, String> {
    generate_counted(model).map(|(schema, _)| schema)
}

/// [`generate`], with the report of [`cmd_schema`]: the counts of classes,
/// enumerations and candidate types.
fn generate_counted(model: &Metamodel) -> Result<(String, String), String> {
    check_model(model)?;

    // Package name -> its classes and enumerations, sorted by name.
    let mut packages: BTreeMap<&str, (Vec<&metamodel::Class>, Vec<&metamodel::Enumeration>)> =
        BTreeMap::new();
    for package in &model.packages {
        let entry = packages.entry(package.name.as_str()).or_default();
        entry.0.extend(&package.classes);
        entry.1.extend(&package.enumerations);
    }

    let mut lines = vec![
        r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_owned(),
        COMMENT.to_owned(),
        format!(
            r#"<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xmi="{XMI_NS}" xmlns:sacm="{SACM_NS}" targetNamespace="{SACM_NS}">"#
        ),
        format!(r#"  <xsd:import namespace="{XMI_NS}" schemaLocation="XMI.xsd"/>"#),
    ];
    let mut unions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let (mut classes, mut enumerations) = (0_usize, 0_usize);
    for (classes_of, enumerations_of) in packages.values_mut() {
        classes_of.sort_by(|a, b| a.name.cmp(&b.name));
        for class in classes_of.iter() {
            let properties: Vec<&Property> = model
                .properties_of(&class.name)
                .into_iter()
                .map(|(_, p)| p)
                .collect();
            complex_type(model, &class.name, &properties, &mut unions, &mut lines)?;
            lines.push(format!(
                r#"  <xsd:element name="{0}" type="sacm:{0}"/>"#,
                class.name
            ));
            classes += 1;
        }
        enumerations_of.sort_by(|a, b| a.name.cmp(&b.name));
        for enumeration in enumerations_of.iter() {
            lines.push(format!(r#"  <xsd:simpleType name="{}">"#, enumeration.name));
            lines.push(r#"    <xsd:restriction base="xsd:string">"#.to_owned());
            for literal in &enumeration.literals {
                lines.push(format!(
                    r#"      <xsd:enumeration value="{}"/>"#,
                    escape(literal)
                ));
            }
            lines.push("    </xsd:restriction>".to_owned());
            lines.push("  </xsd:simpleType>".to_owned());
            enumerations += 1;
        }
    }

    // A candidate type can name further candidate types; each is written
    // once, every round in byte order of name.
    let mut done = BTreeSet::new();
    loop {
        let pending: Vec<String> = unions
            .keys()
            .filter(|t| !done.contains(*t))
            .cloned()
            .collect();
        if pending.is_empty() {
            break;
        }
        for type_name in pending {
            let candidates = unions.get(&type_name).cloned().unwrap_or_default();
            let properties: Vec<&Property> = candidates
                .iter()
                .flat_map(|c| model.properties_of(c))
                .map(|(_, p)| p)
                .collect();
            complex_type(
                model,
                &format!("{type_name}.candidates"),
                &properties,
                &mut unions,
                &mut lines,
            )?;
            done.insert(type_name);
        }
    }
    lines.push("</xsd:schema>".to_owned());

    let mut schema = lines.join("\n");
    schema.push('\n');
    let counts = format!(
        "{}, {}, {}",
        counted(classes, "class", "classes"),
        counted(enumerations, "enumeration", "enumerations"),
        counted(done.len(), "candidate type", "candidate types")
    );
    Ok((schema, counts))
}

/// Refuses a model the generator cannot write a valid schema for.
fn check_model(model: &Metamodel) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for package in &model.packages {
        for class in &package.classes {
            check_name("class", &class.name, &class.name)?;
            if !names.insert(class.name.as_str()) {
                return Err(format!("schema: class {} is declared twice", class.name));
            }
            for property in &class.properties {
                check_name("property", &property.name, &class.name)?;
            }
        }
        for enumeration in &package.enumerations {
            check_name("enumeration", &enumeration.name, &enumeration.name)?;
        }
    }
    for package in &model.packages {
        for class in &package.classes {
            let mut chain = BTreeSet::from([class.name.as_str()]);
            let mut current = class;
            while let Some(superclass) = current.superclass.as_deref() {
                if !chain.insert(superclass) {
                    return Err(format!("schema: class {} is its own ancestor", class.name));
                }
                current = model.class(superclass).ok_or_else(|| {
                    format!(
                        "schema: class {} specializes {superclass}, which is not a class of the \
                         model",
                        current.name
                    )
                })?;
            }
            for property in &class.properties {
                if property.kind == PropertyKind::Composition
                    && model.class(&property.type_name).is_none()
                {
                    return Err(format!(
                        "schema: composition {} of {} is typed by {}, which is not a class of \
                         the model",
                        property.name, class.name, property.type_name
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Refuses a `what` name of `owner` that cannot be an XML element, type or
/// attribute name.
fn check_name(what: &str, name: &str, owner: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "schema: the {what} name {name:?} of {owner} is not an XML name"
        ))
    }
}

/// The properties of one type, merged by name.
struct Merged<'a> {
    /// The names, in order of first appearance.
    names: Vec<&'a str>,
    /// The declaration of each name: its first, unless a later one is a
    /// Composition and it is not.
    by_name: BTreeMap<&'a str, &'a Property>,
    /// The names declared as a Reference, and whether any of them is
    /// multi-valued.
    references: BTreeMap<&'a str, bool>,
}

/// Merges `properties` by name for the type `type_name`.
fn merge<'a>(type_name: &str, properties: &[&'a Property]) -> Result<Merged<'a>, String> {
    let mut merged = Merged {
        names: Vec::new(),
        by_name: BTreeMap::new(),
        references: BTreeMap::new(),
    };
    let mut kinds: BTreeMap<&str, Vec<PropertyKind>> = BTreeMap::new();
    for &property in properties {
        let name = property.name.as_str();
        match merged.by_name.get(name) {
            None => {
                merged.names.push(name);
                merged.by_name.insert(name, property);
            }
            Some(first)
                if first.kind != PropertyKind::Composition
                    && property.kind == PropertyKind::Composition =>
            {
                merged.by_name.insert(name, property);
            }
            Some(_) => {}
        }
        let seen = kinds.entry(name).or_default();
        if !seen.contains(&property.kind) {
            seen.push(property.kind);
        }
        if property.kind == PropertyKind::Reference {
            *merged.references.entry(name).or_default() |= property.upper != Some(1);
        }
    }
    for name in &merged.names {
        let seen = kinds.get(name).map_or(&[][..], Vec::as_slice);
        let composition_and_reference = matches!(
            seen,
            [PropertyKind::Composition, PropertyKind::Reference]
                | [PropertyKind::Reference, PropertyKind::Composition]
        );
        if seen.len() > 1 && !composition_and_reference {
            let kinds: Vec<String> = seen.iter().map(|k| format!("{k:?}")).collect();
            return Err(format!(
                "schema: property {name} of {type_name} is declared as {}",
                kinds.join(" and ")
            ));
        }
    }
    Ok(merged)
}

/// Which content-model group a property's element is written in (rule 4b:
/// attributes, references, compositions).
fn group(kind: PropertyKind) -> u8 {
    match kind {
        PropertyKind::Data | PropertyKind::Enumeration => 0,
        PropertyKind::Reference => 1,
        PropertyKind::Composition => 2,
    }
}

/// Writes the `complexType` `name` declaring `properties` to `lines`, and
/// records in `unions` each candidate type a composition of it needs.
fn complex_type(
    model: &Metamodel,
    name: &str,
    properties: &[&Property],
    unions: &mut BTreeMap<String, Vec<String>>,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    let merged = merge(name, properties)?;
    lines.push(format!(r#"  <xsd:complexType name="{name}">"#));
    lines.push(r#"    <xsd:choice minOccurs="0" maxOccurs="unbounded">"#.to_owned());
    for current in 0..3 {
        for &property_name in &merged.names {
            let Some(property) = merged.by_name.get(property_name) else {
                continue;
            };
            if group(property.kind) != current {
                continue;
            }
            element(model, property, unions, lines);
        }
    }
    lines.push(r#"      <xsd:element ref="xmi:Extension"/>"#.to_owned());
    lines.push(r#"      <xsd:element ref="xmi:extension"/>"#.to_owned());
    lines.push("    </xsd:choice>".to_owned());
    lines.push(r#"    <xsd:attribute ref="xmi:id"/>"#.to_owned());
    lines.push(r#"    <xsd:attributeGroup ref="xmi:ObjectAttribs"/>"#.to_owned());
    for &property_name in &merged.names {
        let Some(property) = merged.by_name.get(property_name) else {
            continue;
        };
        let single = property.upper == Some(1);
        let type_name = if let Some(&multi) = merged.references.get(property_name) {
            if multi { "xsd:IDREFS" } else { "xsd:IDREF" }.to_owned()
        } else if property.kind == PropertyKind::Data && single {
            "xsd:string".to_owned()
        } else if property.kind == PropertyKind::Enumeration && single {
            format!("sacm:{}", property.type_name)
        } else {
            continue;
        };
        lines.push(format!(
            r#"    <xsd:attribute name="{property_name}" type="{type_name}" use="optional"/>"#
        ));
    }
    lines.push("  </xsd:complexType>".to_owned());
    Ok(())
}

/// Writes the element declaration of `property` in a content model.
fn element(
    model: &Metamodel,
    property: &Property,
    unions: &mut BTreeMap<String, Vec<String>>,
    lines: &mut Vec<String>,
) {
    let name = &property.name;
    match property.kind {
        PropertyKind::Data => lines.push(format!(
            r#"      <xsd:element name="{name}" nillable="true" type="xsd:string"/>"#
        )),
        PropertyKind::Enumeration => lines.push(format!(
            r#"      <xsd:element name="{name}" nillable="true" type="sacm:{}"/>"#,
            property.type_name
        )),
        PropertyKind::Reference => {
            lines.push(format!(
                r#"      <xsd:element name="{name}" minOccurs="0" maxOccurs="unbounded">"#
            ));
            lines.push("        <xsd:complexType>".to_owned());
            lines.push(r#"          <xsd:attributeGroup ref="xmi:LinkAttribs"/>"#.to_owned());
            lines.push("        </xsd:complexType>".to_owned());
            lines.push("      </xsd:element>".to_owned());
        }
        PropertyKind::Composition => {
            let candidates = model.candidates(&property.type_name);
            let type_name = if let [only] = candidates.as_slice() {
                (*only).to_owned()
            } else {
                unions.insert(
                    property.type_name.clone(),
                    candidates.iter().map(|&c| c.to_owned()).collect(),
                );
                format!("{}.candidates", property.type_name)
            };
            lines.push(format!(
                r#"      <xsd:element name="{name}" minOccurs="0" maxOccurs="unbounded" type="sacm:{type_name}"/>"#
            ));
        }
    }
}

/// `text` escaped for an attribute value in double quotes.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

/// `n` with the singular or the plural noun.
fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `cargo xtask schema [--out DIR]`: generates the schema from the pinned
/// ptc/22-03-13 under `root` (in `third_party/sacm-2.3/`, where
/// `cargo xtask fetch-spec` puts it), with [`metamodel::CORRECTIONS`]
/// applied, into `out_dir/SACM-2.3.xsd`, and copies the pinned XMI.xsd to
/// `out_dir/XMI.xsd`. Writes:
///
/// ```text
/// wrote <out_dir>/SACM-2.3.xsd (54 classes, 1 enumeration, 8 candidate types)
/// wrote <out_dir>/XMI.xsd
/// ```
///
/// Both inputs are checked against their pins first, the model before
/// XMI.xsd; nothing is written when either is refused.
///
/// # Errors
///
/// Returns a message starting `schema: `:
/// ``schema: third_party/sacm-2.3/<file> is missing; run `cargo xtask fetch-spec` ``,
/// ``schema: third_party/sacm-2.3/<file> has SHA-256 <actual>, expected <pin>; run `cargo xtask fetch-spec` ``,
/// or `schema: <what>` for a read or write error or a model that [`metamodel::parse`],
/// [`Metamodel::corrected`] or [`generate`] refuses. Also an error when
/// writing to `out` fails.
pub fn cmd_schema(root: &Path, out_dir: &Path, out: &mut dyn Write) -> Result<(), String> {
    let model = read_input(root, MODEL_FILE)?;
    let xmi_xsd = read_input(root, XMI_XSD_INPUT)?;
    let text = std::str::from_utf8(&model)
        .map_err(|e| format!("schema: {DEFAULT_DEST}/{MODEL_FILE} is not UTF-8: {e}"))?;
    let model = metamodel::parse(text)
        .and_then(|raw| raw.corrected())
        .map_err(|e| format!("schema: {e}"))?;
    let (schema, counts) = generate_counted(&model)?;

    fs::create_dir_all(out_dir)
        .map_err(|e| format!("schema: create {}: {e}", out_dir.display()))?;
    let schema_path = out_dir.join(SCHEMA_FILE);
    fs::write(&schema_path, schema)
        .map_err(|e| format!("schema: write {}: {e}", schema_path.display()))?;
    writeln!(out, "wrote {} ({counts})", schema_path.display()).map_err(output_error)?;
    let xmi_path = out_dir.join(XMI_SCHEMA_FILE);
    fs::write(&xmi_path, xmi_xsd)
        .map_err(|e| format!("schema: write {}: {e}", xmi_path.display()))?;
    writeln!(out, "wrote {}", xmi_path.display()).map_err(output_error)
}

/// The pinned reference `file` under `root`, its hash checked.
fn read_input(root: &Path, file: &str) -> Result<Vec<u8>, String> {
    metamodel::read_pinned(
        &root.join(DEFAULT_DEST).join(file),
        &format!("{DEFAULT_DEST}/{file}"),
        file,
    )
    .map_err(|e| format!("schema: {e}"))
}

/// One xmllint run: its exit status (`None` when it has none, as when
/// killed by a signal) and what it wrote on stderr.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lint {
    /// The exit status.
    pub status: Option<i32>,
    /// Standard error, decoded lossily as UTF-8.
    pub stderr: String,
}

/// What one xmllint run says about one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The file is valid against the schema.
    Valid,
    /// A [`METADATA_ONLY`] file that fails on XMI-namespace elements only.
    MetadataOnly,
    /// The file is not valid: every `Schemas validity error` line, in order.
    Invalid(Vec<String>),
}

/// Reads the xmllint run `lint` on the file named `file_name`.
///
/// - Exit status 0 is [`Verdict::Valid`], except for a [`METADATA_ONLY`]
///   file, which must fail: then the list entry is stale.
/// - Status 3 or 4 is [`Verdict::Invalid`] with the stderr lines that hold
///   `Schemas validity error`, or, for a [`METADATA_ONLY`] file all of whose
///   such lines are on an element in the XMI namespace,
///   [`Verdict::MetadataOnly`].
///
/// So a listed file cannot hide an error in its SACM content.
///
/// ```
/// use xtask::schema::{Lint, Verdict, judge};
///
/// let lint = Lint { status: Some(0), stderr: "a.sacm.xmi validates\n".to_owned() };
/// assert_eq!(judge("a.sacm.xmi", &lint), Ok(Verdict::Valid));
/// ```
///
/// # Errors
///
/// Returns `<file> is listed in METADATA_ONLY but is valid; remove it from
/// the list` for a listed file that is valid, and
/// `xmllint failed on <file> (exit <status>): <first stderr line>` for any
/// other status, no status, or status 3 or 4 without a validity error.
pub fn judge(file_name: &str, lint: &Lint) -> Result<Verdict, String> {
    let listed = METADATA_ONLY
        .iter()
        .any(|m| file_name == *m || file_name.ends_with(&format!("__{m}")));
    match lint.status {
        Some(0) if listed => Err(format!(
            "{file_name} is listed in METADATA_ONLY but is valid; remove it from the list"
        )),
        Some(0) => Ok(Verdict::Valid),
        Some(3 | 4) => {
            let errors: Vec<String> = lint
                .stderr
                .lines()
                .filter(|l| l.contains(VALIDITY_ERROR))
                .map(str::to_owned)
                .collect();
            if errors.is_empty() {
                Err(lint_failure(file_name, lint))
            } else if listed && errors.iter().all(|e| e.contains(XMI_ELEMENT_ERROR)) {
                Ok(Verdict::MetadataOnly)
            } else {
                Ok(Verdict::Invalid(errors))
            }
        }
        _ => Err(lint_failure(file_name, lint)),
    }
}

/// The message for an xmllint run that did not judge `file_name`.
fn lint_failure(file_name: &str, lint: &Lint) -> String {
    let status = lint
        .status
        .map_or_else(|| "none".to_owned(), |s| s.to_string());
    let first = lint.stderr.lines().next().unwrap_or("(no output)");
    format!("xmllint failed on {file_name} (exit {status}): {first}")
}

/// Checks every strict golden file in `root/GOLDEN_DIR` and then every file
/// in `out_dir/outputs`, each set in byte order of file name, by calling
/// `lint(out_dir/SACM-2.3.xsd, file)` and [`judge`] on each.
///
/// Writes one line per file, `ok golden/<name>`, `ok outputs/<name>` or
/// `metadata <label>` (`invalid <label>` and `error <label>` for a file
/// that fails), then, when nothing failed,
/// `schema: <G> golden files and <O> outputs checked: <V> valid, <M> invalid in XMI metadata only`.
///
/// # Errors
///
/// Every file is checked first. Then the error holds every problem, one per
/// line: `<N> of <T> files are not valid against the SACM XML Schema`
/// followed by `<label>: <xmllint line>` for each error; each error of
/// [`judge`]; `no golden files in <GOLDEN_DIR>`; and
/// `fewer outputs (<O>) than golden files (<G>) in <out_dir>/outputs; run the strict-output step first`,
/// so the check cannot pass on missing outputs. A `lint` error (xmllint
/// cannot run), a directory that cannot be read, or a failed write to `out`
/// stops the check at once.
pub fn validate_with(
    root: &Path,
    out_dir: &Path,
    lint: &mut dyn FnMut(&Path, &Path) -> Result<Lint, String>,
    out: &mut dyn Write,
) -> Result<(), String> {
    let goldens = files_in(&root.join(GOLDEN_DIR), |name| {
        name.ends_with(".golden.sacm.xmi")
    })?;
    let outputs_dir = out_dir.join(OUTPUTS_DIR);
    let outputs = files_in(&outputs_dir, |_| true)?;
    let schema = out_dir.join(SCHEMA_FILE);

    let (mut valid, mut metadata) = (0_usize, 0_usize);
    let mut invalid: Vec<String> = Vec::new();
    let mut invalid_files = 0_usize;
    let mut problems: Vec<String> = Vec::new();
    let files = goldens
        .iter()
        .map(|(name, path)| (format!("golden/{name}"), name, path))
        .chain(
            outputs
                .iter()
                .map(|(name, path)| (format!("outputs/{name}"), name, path)),
        );
    for (label, name, path) in files {
        let result = lint(&schema, path)?;
        let report = match judge(name, &result) {
            Ok(Verdict::Valid) => {
                valid += 1;
                format!("ok {label}")
            }
            Ok(Verdict::MetadataOnly) => {
                metadata += 1;
                format!("metadata {label}")
            }
            Ok(Verdict::Invalid(errors)) => {
                invalid_files += 1;
                invalid.extend(errors.iter().map(|e| format!("{label}: {e}")));
                format!("invalid {label}")
            }
            Err(problem) => {
                problems.push(problem);
                format!("error {label}")
            }
        };
        writeln!(out, "{report}").map_err(output_error)?;
    }

    let mut errors = Vec::new();
    if invalid_files > 0 {
        errors.push(format!(
            "{invalid_files} of {} files are not valid against the SACM XML Schema",
            goldens.len() + outputs.len()
        ));
        errors.append(&mut invalid);
    }
    errors.append(&mut problems);
    if goldens.is_empty() {
        errors.push(format!("no golden files in {GOLDEN_DIR}"));
    } else if outputs.len() < goldens.len() {
        errors.push(format!(
            "fewer outputs ({}) than golden files ({}) in {}; run the strict-output step first",
            outputs.len(),
            goldens.len(),
            outputs_dir.display()
        ));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    writeln!(
        out,
        "schema: {} golden files and {} outputs checked: {valid} valid, {metadata} invalid in \
         XMI metadata only",
        goldens.len(),
        outputs.len()
    )
    .map_err(output_error)
}

/// The files in `dir` whose names pass `keep`, by name in byte order, with
/// their paths. A missing directory has none.
fn files_in(dir: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<(String, PathBuf)>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read {}: {e}", dir.display())),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = dir.join(&name);
        if path.is_file() && keep(&name) {
            files.push((name, path));
        }
    }
    files.sort();
    Ok(files)
}

/// Runs `xmllint --noout --nonet --schema <schema> <file>`.
///
/// # Errors
///
/// Returns `could not run xmllint (is libxml2-utils installed?): <error>`
/// when xmllint cannot be started.
pub fn xmllint(schema: &Path, file: &Path) -> Result<Lint, String> {
    let output = Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(schema)
        .arg(file)
        .output()
        .map_err(spawn_error)?;
    Ok(Lint {
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// The message for an xmllint that cannot be started.
// Takes the error by value so that it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn spawn_error(error: std::io::Error) -> String {
    format!("could not run xmllint (is libxml2-utils installed?): {error}")
}

/// `cargo xtask schema validate [--out DIR]`: [`cmd_schema`], then the first
/// line of `xmllint --version` as xmllint writes it (it names the tool and
/// the libxml2 version), then [`validate_with`] with [`xmllint`].
///
/// # Errors
///
/// As [`cmd_schema`] and [`validate_with`], and when xmllint cannot be run.
pub fn cmd_validate(root: &Path, out_dir: &Path, out: &mut dyn Write) -> Result<(), String> {
    cmd_schema(root, out_dir, out)?;
    let version = Command::new("xmllint")
        .arg("--version")
        .output()
        .map_err(spawn_error)?;
    let text = if version.stderr.is_empty() {
        version.stdout
    } else {
        version.stderr
    };
    let text = String::from_utf8_lossy(&text);
    writeln!(out, "{}", text.lines().next().unwrap_or("").trim_end()).map_err(output_error)?;
    validate_with(root, out_dir, &mut xmllint, out)
}
