//! The SACM XML Schema that `cargo xtask schema` generates, and the check of
//! strict output against it (LIB-TOOL-023, ADR 0043).
//!
//! `xtask::schema::generate` applies the XMI 2.5.1 clause 8 rules, with the
//! default tag values, to a `xtask::metamodel::Metamodel`, with rule 4f's
//! candidate list replaced by a candidate type and rule 7 not applied.
//! `cmd_schema` writes it and a copy of the pinned XMI.xsd. `judge` reads one
//! xmllint run, and `validate_with` checks every strict golden file and every
//! strict output with a given lint, so these tests need no xmllint: CI's
//! `SACM XML Schema` job runs the real one (`cmd_validate`).
//!
//! Tests 1 to 5 use the design brief's sample model (`common/sample.rs`) and
//! its schema, `SAMPLE_SCHEMA` below. Tests 6 and 7 need the pinned model:
//! see `common/spec.rs` for where it is found and when a test is skipped.

#[path = "common/sample.rs"]
mod sample;
#[path = "common/spec.rs"]
mod spec;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use xtask::fetch::{ARTIFACTS, DEFAULT_DEST, sha256_hex};
use xtask::metamodel::{MODEL_FILE, Metamodel, load, parse};
use xtask::schema::{
    DEFAULT_OUT, GENERATED_SHA256, GOLDEN_DIR, Lint, METADATA_ONLY, OUTPUTS_DIR, SCHEMA_FILE,
    Verdict, XMI_SCHEMA_FILE, cmd_schema, generate, judge, validate_with,
};

use sample::SAMPLE_MODEL;
use spec::spec_dir;

const ROW: &str = "LIB-TOOL-023";

/// The pinned XMI.xsd, as `fetch-spec` names it.
const XMI_XSD: &str = "XMI-2.5.1-XMI.xsd";

/// The fetch command, quoted in the refusals.
const FETCH: &str = "run `cargo xtask fetch-spec`";

/// The four header lines of every generated schema.
const HEADER: [&str; 4] = [
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<!-- The SACM 2.3 XML Schema as libSACM generates it (cargo xtask schema) from OMG \
     ptc/22-03-13 by the XMI 2.5.1 clause 8 rules, with the changes listed in \
     docs/sacm/compliance-points.md. Not an OMG document. -->",
    r#"<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:sacm="http://www.omg.org/spec/SACM/20220301" targetNamespace="http://www.omg.org/spec/SACM/20220301">"#,
    r#"  <xsd:import namespace="http://www.omg.org/spec/XMI/20131001" schemaLocation="XMI.xsd"/>"#,
];

/// The SHA-256 of [`SAMPLE_SCHEMA`], as the design brief pins it.
const SAMPLE_SCHEMA_SHA256: &str =
    "5a097b8ac180c4f8cac7ca3f3ff4c665c6b5c558cbc8184d131f79a17a2cabec";

/// The schema of `SAMPLE_MODEL`, byte for byte: what the design brief's
/// reference generator writes for it.
const SAMPLE_SCHEMA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- The SACM 2.3 XML Schema as libSACM generates it (cargo xtask schema) from OMG ptc/22-03-13 by the XMI 2.5.1 clause 8 rules, with the changes listed in docs/sacm/compliance-points.md. Not an OMG document. -->
<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:sacm="http://www.omg.org/spec/SACM/20220301" targetNamespace="http://www.omg.org/spec/SACM/20220301">
  <xsd:import namespace="http://www.omg.org/spec/XMI/20131001" schemaLocation="XMI.xsd"/>
  <xsd:complexType name="Box">
    <xsd:choice minOccurs="0" maxOccurs="unbounded">
      <xsd:element name="label" nillable="true" type="xsd:string"/>
      <xsd:element name="next" minOccurs="0" maxOccurs="unbounded">
        <xsd:complexType>
          <xsd:attributeGroup ref="xmi:LinkAttribs"/>
        </xsd:complexType>
      </xsd:element>
      <xsd:element name="item" minOccurs="0" maxOccurs="unbounded" type="sacm:Item.candidates"/>
      <xsd:element name="tag" minOccurs="0" maxOccurs="unbounded" type="sacm:Tag"/>
      <xsd:element ref="xmi:Extension"/>
      <xsd:element ref="xmi:extension"/>
    </xsd:choice>
    <xsd:attribute ref="xmi:id"/>
    <xsd:attributeGroup ref="xmi:ObjectAttribs"/>
    <xsd:attribute name="label" type="xsd:string" use="optional"/>
    <xsd:attribute name="next" type="xsd:IDREFS" use="optional"/>
  </xsd:complexType>
  <xsd:element name="Box" type="sacm:Box"/>
  <xsd:complexType name="Item">
    <xsd:choice minOccurs="0" maxOccurs="unbounded">
      <xsd:element name="label" nillable="true" type="xsd:string"/>
      <xsd:element ref="xmi:Extension"/>
      <xsd:element ref="xmi:extension"/>
    </xsd:choice>
    <xsd:attribute ref="xmi:id"/>
    <xsd:attributeGroup ref="xmi:ObjectAttribs"/>
    <xsd:attribute name="label" type="xsd:string" use="optional"/>
  </xsd:complexType>
  <xsd:element name="Item" type="sacm:Item"/>
  <xsd:complexType name="Leaf">
    <xsd:choice minOccurs="0" maxOccurs="unbounded">
      <xsd:element name="label" nillable="true" type="xsd:string"/>
      <xsd:element name="state" nillable="true" type="sacm:State"/>
      <xsd:element name="next" minOccurs="0" maxOccurs="unbounded">
        <xsd:complexType>
          <xsd:attributeGroup ref="xmi:LinkAttribs"/>
        </xsd:complexType>
      </xsd:element>
      <xsd:element name="item" minOccurs="0" maxOccurs="unbounded">
        <xsd:complexType>
          <xsd:attributeGroup ref="xmi:LinkAttribs"/>
        </xsd:complexType>
      </xsd:element>
      <xsd:element ref="xmi:Extension"/>
      <xsd:element ref="xmi:extension"/>
    </xsd:choice>
    <xsd:attribute ref="xmi:id"/>
    <xsd:attributeGroup ref="xmi:ObjectAttribs"/>
    <xsd:attribute name="label" type="xsd:string" use="optional"/>
    <xsd:attribute name="state" type="sacm:State" use="optional"/>
    <xsd:attribute name="next" type="xsd:IDREF" use="optional"/>
    <xsd:attribute name="item" type="xsd:IDREF" use="optional"/>
  </xsd:complexType>
  <xsd:element name="Leaf" type="sacm:Leaf"/>
  <xsd:complexType name="Tag">
    <xsd:choice minOccurs="0" maxOccurs="unbounded">
      <xsd:element ref="xmi:Extension"/>
      <xsd:element ref="xmi:extension"/>
    </xsd:choice>
    <xsd:attribute ref="xmi:id"/>
    <xsd:attributeGroup ref="xmi:ObjectAttribs"/>
  </xsd:complexType>
  <xsd:element name="Tag" type="sacm:Tag"/>
  <xsd:simpleType name="State">
    <xsd:restriction base="xsd:string">
      <xsd:enumeration value="open"/>
      <xsd:enumeration value="closed"/>
    </xsd:restriction>
  </xsd:simpleType>
  <xsd:complexType name="Item.candidates">
    <xsd:choice minOccurs="0" maxOccurs="unbounded">
      <xsd:element name="label" nillable="true" type="xsd:string"/>
      <xsd:element name="state" nillable="true" type="sacm:State"/>
      <xsd:element name="next" minOccurs="0" maxOccurs="unbounded">
        <xsd:complexType>
          <xsd:attributeGroup ref="xmi:LinkAttribs"/>
        </xsd:complexType>
      </xsd:element>
      <xsd:element name="item" minOccurs="0" maxOccurs="unbounded" type="sacm:Item.candidates"/>
      <xsd:element name="tag" minOccurs="0" maxOccurs="unbounded" type="sacm:Tag"/>
      <xsd:element ref="xmi:Extension"/>
      <xsd:element ref="xmi:extension"/>
    </xsd:choice>
    <xsd:attribute ref="xmi:id"/>
    <xsd:attributeGroup ref="xmi:ObjectAttribs"/>
    <xsd:attribute name="label" type="xsd:string" use="optional"/>
    <xsd:attribute name="item" type="xsd:IDREF" use="optional"/>
    <xsd:attribute name="next" type="xsd:IDREFS" use="optional"/>
    <xsd:attribute name="state" type="sacm:State" use="optional"/>
  </xsd:complexType>
</xsd:schema>
"#;

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

    /// Writes `bytes` at the `/`-separated `rel`, creating its directories.
    fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("{ROW}: setup: create {}: {e}", parent.display()));
        }
        fs::write(&path, bytes)
            .unwrap_or_else(|e| panic!("{ROW}: setup: write {}: {e}", path.display()));
    }

    /// Creates the `/`-separated directory `rel`.
    fn mkdir(&self, rel: &str) {
        let path = self.0.join(rel);
        fs::create_dir_all(&path)
            .unwrap_or_else(|e| panic!("{ROW}: setup: create {}: {e}", path.display()));
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

/// `model`'s schema, or the test fails.
fn generated(what: &str, model: &Metamodel) -> String {
    generate(model).unwrap_or_else(|e| panic!("{ROW}: generate({what}) fails: {e}"))
}

/// The trimmed lines of the `complexType` named `name`, from its start tag to
/// its end tag; empty when there is none.
fn complex_type<'a>(schema: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!(r#"<xsd:complexType name="{name}">"#);
    schema
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != start)
        // The depth of nested complexTypes: a reference element holds an
        // anonymous one, so only the end tag back at depth 0 ends the type.
        .scan(Some(0_usize), |depth, line| {
            let current = (*depth)?;
            let current = if line.starts_with("<xsd:complexType") && !line.ends_with("/>") {
                current + 1
            } else if line == "</xsd:complexType>" {
                current.saturating_sub(1)
            } else {
                current
            };
            *depth = (current > 0).then_some(current);
            Some(line)
        })
        .collect()
}

/// The pin of the artifact `name`.
fn pin(name: &str) -> &'static str {
    ARTIFACTS.iter().find(|a| a.file_name == name).map_or_else(
        || panic!("{ROW}: setup: {name} is not in ARTIFACTS"),
        |a| a.sha256,
    )
}

/// The SHA-256 of the file at `path`.
fn file_sha256(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{ROW}: read {}: {e}", path.display()));
    sha256_hex(&bytes)
}

/// The first line where `actual` and `expected` differ, for a message.
fn first_difference(actual: &str, expected: &str) -> String {
    let line = actual
        .lines()
        .zip(expected.lines())
        .position(|(a, e)| a != e)
        .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
    format!(
        "line {}: got {:?}, expected {:?}",
        line + 1,
        actual.lines().nth(line),
        expected.lines().nth(line)
    )
}

// ---------------------------------------------------------------------------
// 1 to 5: generate, on hand-written models
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_023_the_sample_model_gives_the_sample_schema() {
    assert_eq!(
        sha256_hex(SAMPLE_SCHEMA.as_bytes()),
        SAMPLE_SCHEMA_SHA256,
        "{ROW}: setup: SAMPLE_SCHEMA is the design brief's (its SHA-256)"
    );
    let schema = generated(
        "the sample model",
        &parsed("the sample model", SAMPLE_MODEL),
    );
    assert!(
        schema == SAMPLE_SCHEMA,
        "{ROW}: the schema of SAMPLE_MODEL is not SAMPLE_SCHEMA byte for byte; first difference \
         at {}",
        first_difference(&schema, SAMPLE_SCHEMA)
    );
}

#[test]
fn lib_tool_023_generation_is_deterministic_text() {
    let first = generated(
        "the sample model",
        &parsed("the sample model", SAMPLE_MODEL),
    );
    // Each parse and each generation builds its own maps, so an output that
    // followed a hash order would differ between rounds.
    for round in 0..8 {
        let again = generated(
            "the sample model",
            &parsed("the sample model", SAMPLE_MODEL),
        );
        assert!(
            again == first,
            "{ROW}: generation round {round} differs from the first at {}",
            first_difference(&again, &first)
        );
    }
    assert!(
        !first.contains('\r'),
        "{ROW}: the schema has LF line ends only"
    );
    assert!(!first.contains('\t'), "{ROW}: the schema has no tab");
    for (n, line) in first.lines().enumerate() {
        assert!(
            !line.ends_with(' '),
            "{ROW}: line {} ends in a space: {line:?}",
            n + 1
        );
    }
    assert!(
        first.ends_with('\n') && !first.ends_with("\n\n"),
        "{ROW}: the schema ends with exactly one newline"
    );
    let header: Vec<&str> = first.lines().take(4).collect();
    assert_eq!(header, HEADER, "{ROW}: the four header lines");
}

/// Compositions typed by a concrete class with subclasses (`A`), by a leaf
/// class (`D`) and by an abstract class with one concrete subclass
/// (`Solo`). `C` is written before `B`, so the candidates' byte order is not
/// the document order.
const CANDIDATES_MODEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <uml:Model xmi:type="uml:Model" xmi:id="m" name="Model">
    <packagedElement xmi:type="uml:Package" xmi:id="p" name="Sample">
      <packagedElement xmi:type="uml:Class" xmi:id="Holder" name="Holder">
        <ownedAttribute xmi:type="uml:Property" xmi:id="Holder.part" name="part" type="A" aggregation="composite"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Holder.leaf" name="leaf" type="D" aggregation="composite"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Holder.solo" name="solo" type="Solo" aggregation="composite"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="C" name="C">
        <generalization xmi:type="uml:Generalization" xmi:id="C.general" general="A"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="A" name="A"/>
      <packagedElement xmi:type="uml:Class" xmi:id="B" name="B">
        <generalization xmi:type="uml:Generalization" xmi:id="B.general" general="A"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="D" name="D">
        <generalization xmi:type="uml:Generalization" xmi:id="D.general" general="B"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="Solo" name="Solo" isAbstract="true"/>
      <packagedElement xmi:type="uml:Class" xmi:id="Only" name="Only">
        <generalization xmi:type="uml:Generalization" xmi:id="Only.general" general="Solo"/>
      </packagedElement>
    </packagedElement>
  </uml:Model>
</xmi:XMI>
"#;

#[test]
fn lib_tool_023_a_composition_is_typed_by_its_one_candidate_or_by_a_candidate_type() {
    let sample = parsed("the sample model", SAMPLE_MODEL);
    assert_eq!(
        sample.candidates("Item"),
        ["Box", "Leaf"],
        "{ROW}: candidates(Item)"
    );
    assert_eq!(sample.candidates("Tag"), ["Tag"], "{ROW}: candidates(Tag)");
    let schema = generated("the sample model", &sample);
    let boxed = complex_type(&schema, "Box");
    for line in [
        r#"<xsd:element name="tag" minOccurs="0" maxOccurs="unbounded" type="sacm:Tag"/>"#,
        r#"<xsd:element name="item" minOccurs="0" maxOccurs="unbounded" type="sacm:Item.candidates"/>"#,
    ] {
        assert!(
            boxed.contains(&line),
            "{ROW}: Box declares {line}; it is {boxed:#?}"
        );
    }
    assert!(
        !schema.contains(r#"<xsd:element ref="sacm:"#),
        "{ROW}: rule 4f's child-element form is never generated"
    );
    assert_eq!(
        schema
            .matches(r#"<xsd:complexType name="Item.candidates">"#)
            .count(),
        1,
        "{ROW}: Item.candidates is declared once"
    );
    assert!(
        !schema.contains("Tag.candidates"),
        "{ROW}: one candidate needs no candidate type"
    );

    let model = parsed("the candidates model", CANDIDATES_MODEL);
    assert_eq!(
        model.candidates("A"),
        ["A", "B", "D", "C"],
        "{ROW}: the candidates of A: A, as it is concrete, then its immediate subclasses in byte \
         order, depth first"
    );
    assert_eq!(
        model.candidates("Solo"),
        ["Only"],
        "{ROW}: candidates(Solo)"
    );
    let schema = generated("the candidates model", &model);
    let holder = complex_type(&schema, "Holder");
    for line in [
        r#"<xsd:element name="part" minOccurs="0" maxOccurs="unbounded" type="sacm:A.candidates"/>"#,
        r#"<xsd:element name="leaf" minOccurs="0" maxOccurs="unbounded" type="sacm:D"/>"#,
        r#"<xsd:element name="solo" minOccurs="0" maxOccurs="unbounded" type="sacm:Only"/>"#,
    ] {
        assert!(
            holder.contains(&line),
            "{ROW}: Holder declares {line}; it is {holder:#?}"
        );
    }
    assert!(
        !complex_type(&schema, "A.candidates").is_empty() && !schema.contains("Solo.candidates"),
        "{ROW}: A has a candidate type, and Solo, with one candidate, none"
    );
}

#[test]
fn lib_tool_023_a_candidate_type_merges_properties_of_one_name() {
    let schema = generated(
        "the sample model",
        &parsed("the sample model", SAMPLE_MODEL),
    );
    let union = complex_type(&schema, "Item.candidates");
    // `item` is a Composition of Box and a Reference of Leaf: the
    // composition's element and the reference's attribute.
    let item_element = r#"<xsd:element name="item" minOccurs="0" maxOccurs="unbounded" type="sacm:Item.candidates"/>"#;
    let item_attribute = r#"<xsd:attribute name="item" type="xsd:IDREF" use="optional"/>"#;
    // `next` is a Reference of both, single-valued in Leaf and multi-valued
    // in Box: IDREFS, and the reference element.
    let next_element = r#"<xsd:element name="next" minOccurs="0" maxOccurs="unbounded">"#;
    let next_attribute = r#"<xsd:attribute name="next" type="xsd:IDREFS" use="optional"/>"#;
    for line in [item_element, item_attribute, next_element, next_attribute] {
        assert_eq!(
            union.iter().filter(|l| **l == line).count(),
            1,
            "{ROW}: Item.candidates declares {line} once; it is {union:#?}"
        );
    }
    for name in ["item", "next", "label", "state", "tag"] {
        let elements = union
            .iter()
            .filter(|l| l.starts_with(&format!(r#"<xsd:element name="{name}""#)))
            .count();
        assert_eq!(
            elements, 1,
            "{ROW}: Item.candidates has one element named {name}"
        );
    }
    let next_at = union.iter().position(|l| *l == next_element).unwrap_or(0);
    assert_eq!(
        union.get(next_at + 2),
        Some(&r#"<xsd:attributeGroup ref="xmi:LinkAttribs"/>"#),
        "{ROW}: the `next` element of Item.candidates is the reference form"
    );
    assert!(
        complex_type(&schema, "Leaf")
            .contains(&r#"<xsd:attribute name="next" type="xsd:IDREF" use="optional"/>"#),
        "{ROW}: Leaf's own `next` is IDREF, so IDREFS in Item.candidates is the merge"
    );
}

/// A candidate type whose two candidates declare `x` as Data and as an
/// Enumeration.
const CONFLICT_MODEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <uml:Model xmi:type="uml:Model" xmi:id="m" name="Model">
    <packagedElement xmi:type="uml:Package" xmi:id="p" name="Sample">
      <packagedElement xmi:type="uml:Class" xmi:id="Holder" name="Holder">
        <ownedAttribute xmi:type="uml:Property" xmi:id="Holder.t" name="t" type="T" aggregation="composite"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="T" name="T" isAbstract="true"/>
      <packagedElement xmi:type="uml:Class" xmi:id="A" name="A">
        <generalization xmi:type="uml:Generalization" xmi:id="A.general" general="T"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="A.x" name="x">
          <type href="http://www.omg.org/spec/UML/20131001/PrimitiveTypes.xmi#String"/>
        </ownedAttribute>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="B" name="B">
        <generalization xmi:type="uml:Generalization" xmi:id="B.general" general="T"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="B.x" name="x" type="E"/>
      </packagedElement>
      <packagedElement xmi:type="uml:Enumeration" xmi:id="E" name="E">
        <ownedLiteral xmi:type="uml:EnumerationLiteral" xmi:id="E.e1" name="e1"/>
      </packagedElement>
    </packagedElement>
  </uml:Model>
</xmi:XMI>
"#;

#[test]
fn lib_tool_023_conflicting_properties_in_a_candidate_type_are_an_error() {
    let model = parsed("the conflict model", CONFLICT_MODEL);
    assert_eq!(
        generate(&model),
        Err("schema: property x of T.candidates is declared as Data and Enumeration".to_owned()),
        "{ROW}: two candidates declaring x as Data and as Enumeration are an error, naming the \
         property, the candidate type and both kinds in candidate order"
    );
    // Control: the same model with both declared as Data generates.
    let agreeing = CONFLICT_MODEL.replace(
        r#"name="x" type="E"/>"#,
        r#"name="x"><type href="http://www.omg.org/spec/UML/20131001/PrimitiveTypes.xmi#String"/></ownedAttribute>"#,
    );
    let schema = generated(
        "the agreeing model",
        &parsed("the agreeing model", &agreeing),
    );
    assert!(
        complex_type(&schema, "T.candidates")
            .contains(&r#"<xsd:attribute name="x" type="xsd:string" use="optional"/>"#),
        "{ROW}: two candidates declaring x alike give one declaration"
    );
}

// ---------------------------------------------------------------------------
// 6 to 8: the pinned model and `cmd_schema`
// ---------------------------------------------------------------------------

/// The eight candidate types of SACM 2.3.
const CANDIDATE_TYPES: [&str; 8] = [
    "ArgumentPackage",
    "ArgumentationElement",
    "ArtifactElement",
    "ArtifactPackage",
    "AssuranceCasePackage",
    "LangString",
    "TerminologyElement",
    "TerminologyPackage",
];

#[test]
fn lib_tool_023_the_schema_of_the_pinned_model_has_the_pinned_hash() {
    assert_eq!(
        GENERATED_SHA256, "2dde06a5d97d8c855873092b5a0d37bb8be79ea738412673d55fce56637792cc",
        "{ROW}: GENERATED_SHA256 is the design brief's pin"
    );
    let Some(dir) = spec_dir(ROW, &[MODEL_FILE]) else {
        return;
    };
    let model = load(&dir)
        .and_then(|raw| raw.corrected())
        .unwrap_or_else(|e| panic!("{ROW}: the pinned model does not load and correct: {e}"));
    let schema = generated("the pinned model", &model);
    assert_eq!(
        sha256_hex(schema.as_bytes()),
        GENERATED_SHA256,
        "{ROW}: the schema of the corrected ptc/22-03-13 has the pinned SHA-256"
    );
    assert_eq!(schema.lines().count(), 2_589, "{ROW}: 2,589 lines");
    assert_eq!(schema.len(), 145_742, "{ROW}: 145,742 bytes");
    assert_eq!(
        schema.matches("<xsd:complexType name=").count(),
        62,
        "{ROW}: 62 named complex types: 54 classes and 8 candidate types"
    );
    let candidates: Vec<&str> = schema
        .lines()
        .filter_map(|l| l.trim().strip_prefix(r#"<xsd:complexType name=""#))
        .filter_map(|l| l.strip_suffix(r#".candidates">"#))
        .collect();
    assert_eq!(
        candidates, CANDIDATE_TYPES,
        "{ROW}: the eight candidate types, by name"
    );
}

/// A root holding the pinned `files` from `dir` where `fetch-spec` puts them.
fn root_with_spec(test: &str, dir: &Path, files: &[&str]) -> TempDir {
    let root = TempDir::new(test);
    for name in files {
        let from = dir.join(name);
        let bytes = fs::read(&from)
            .unwrap_or_else(|e| panic!("{ROW}: setup: read {}: {e}", from.display()));
        root.write(&format!("{DEFAULT_DEST}/{name}"), &bytes);
    }
    root
}

/// Runs `cmd_schema`, returning its result and what it wrote.
fn run_cmd_schema(root: &Path, out_dir: &Path) -> (Result<(), String>, String) {
    let mut out = Vec::new();
    let result = cmd_schema(root, out_dir, &mut out);
    let text =
        String::from_utf8(out).unwrap_or_else(|e| panic!("{ROW}: cmd_schema wrote non-UTF-8: {e}"));
    (result, text)
}

#[test]
fn lib_tool_023_cmd_schema_writes_the_schema_and_the_xmi_xsd_copy() {
    assert_eq!(
        (DEFAULT_OUT, SCHEMA_FILE, XMI_SCHEMA_FILE),
        ("target/schema", "SACM-2.3.xsd", "XMI.xsd"),
        "{ROW}: the default out dir and the two file names"
    );
    let ignore = fs::read_to_string(xtask::repo_root().join(".gitignore"))
        .unwrap_or_else(|e| panic!("{ROW}: read .gitignore: {e}"));
    assert!(
        ignore
            .lines()
            .any(|l| l.trim() == "/target" || l.trim() == "/target/"),
        "{ROW}: target/ is git-ignored, so target/schema is never committed"
    );
    let Some(dir) = spec_dir(ROW, &[MODEL_FILE, XMI_XSD]) else {
        return;
    };
    let root = root_with_spec("lib-tool-023-cmd-schema", &dir, &[MODEL_FILE, XMI_XSD]);
    // Not there yet: the command creates it.
    let out_dir = root.path().join("out").join("schema");
    let expected = format!(
        "wrote {} (54 classes, 1 enumeration, 8 candidate types)\nwrote {}\n",
        out_dir.join(SCHEMA_FILE).display(),
        out_dir.join(XMI_SCHEMA_FILE).display()
    );
    for round in ["first", "second"] {
        let (result, text) = run_cmd_schema(root.path(), &out_dir);
        assert_eq!(result, Ok(()), "{ROW}: the {round} cmd_schema succeeds");
        assert_eq!(
            text, expected,
            "{ROW}: the {round} cmd_schema reports both files"
        );
        assert_eq!(
            file_sha256(&out_dir.join(SCHEMA_FILE)),
            GENERATED_SHA256,
            "{ROW}: the {round} {SCHEMA_FILE} has the pinned SHA-256"
        );
        assert_eq!(
            file_sha256(&out_dir.join(XMI_SCHEMA_FILE)),
            pin(XMI_XSD),
            "{ROW}: the {round} {XMI_SCHEMA_FILE} is the pinned XMI.xsd"
        );
    }

    // The XMI.xsd is checked against its pin before use, too.
    let changed = b"junk: not the pinned XMI.xsd";
    root.write(&format!("{DEFAULT_DEST}/{XMI_XSD}"), changed);
    let other_out = root.path().join("other");
    assert_eq!(
        run_cmd_schema(root.path(), &other_out).0,
        Err(format!(
            "schema: {DEFAULT_DEST}/{XMI_XSD} has SHA-256 {}, expected {}; {FETCH}",
            sha256_hex(changed),
            pin(XMI_XSD)
        )),
        "{ROW}: a changed XMI.xsd is refused, naming fetch-spec"
    );
    fs::remove_file(root.path().join(DEFAULT_DEST).join(XMI_XSD))
        .unwrap_or_else(|e| panic!("{ROW}: setup: remove {XMI_XSD}: {e}"));
    assert_eq!(
        run_cmd_schema(root.path(), &other_out).0,
        Err(format!(
            "schema: {DEFAULT_DEST}/{XMI_XSD} is missing; {FETCH}"
        )),
        "{ROW}: a missing XMI.xsd is refused, naming fetch-spec"
    );
    assert!(
        !other_out.exists(),
        "{ROW}: nothing is written when an input is refused"
    );
}

#[test]
fn lib_tool_023_cmd_schema_names_fetch_spec_when_the_model_is_missing_or_changed() {
    let root = TempDir::new("lib-tool-023-no-model");
    let out_dir = root.path().join(DEFAULT_OUT);
    assert_eq!(
        run_cmd_schema(root.path(), &out_dir).0,
        Err(format!(
            "schema: {DEFAULT_DEST}/{MODEL_FILE} is missing; {FETCH}"
        )),
        "{ROW}: an empty root: the model is missing, and fetch-spec is named"
    );
    let changed = b"junk: not the pinned ptc/22-03-13";
    root.write(&format!("{DEFAULT_DEST}/{MODEL_FILE}"), changed);
    assert_eq!(
        run_cmd_schema(root.path(), &out_dir).0,
        Err(format!(
            "schema: {DEFAULT_DEST}/{MODEL_FILE} has SHA-256 {}, expected {}; {FETCH}",
            sha256_hex(changed),
            pin(MODEL_FILE)
        )),
        "{ROW}: a changed model: both hashes, and fetch-spec is named"
    );
    assert!(
        !out_dir.exists(),
        "{ROW}: nothing is written when the model is refused"
    );
}

// ---------------------------------------------------------------------------
// 9 and 10: judge
// ---------------------------------------------------------------------------

/// xmllint's line for an error on an XMI-namespace element, as libxml2 2.12
/// writes it for `<xmi:exporter>` in `xmi:documentation`.
fn xmi_error(file: &str) -> String {
    format!(
        "{file}:3: element exporter: Schemas validity error : Element \
         '{{http://www.omg.org/spec/XMI/20131001}}exporter': This element is not expected. \
         Expected is one of ( contact, exporter, exporterVersion, longDescription, \
         shortDescription, notice, owner, timestamp, {{http://www.omg.org/spec/XMI/20131001}}Extension )."
    )
}

/// xmllint's line for an error on a SACM role element.
fn sacm_error(file: &str, line: u32) -> String {
    format!(
        "{file}:{line}: element argumentElement: Schemas validity error : Element \
         'argumentElement', attribute 'colour': The attribute 'colour' is not allowed."
    )
}

/// What xmllint writes on stderr for a file with `errors`.
fn failing(file: &str, errors: &[String]) -> Lint {
    let mut lines: Vec<String> = errors.to_vec();
    lines.push(format!("{file} fails to validate"));
    let mut stderr = lines.join("\n");
    stderr.push('\n');
    Lint {
        status: Some(3),
        stderr,
    }
}

/// What xmllint writes for a valid file.
fn passing(file: &str) -> Lint {
    Lint {
        status: Some(0),
        stderr: format!("{file} validates\n"),
    }
}

#[test]
fn lib_tool_023_judge_reads_xmllint_exit_codes() {
    let file = "single-root.golden.sacm.xmi";
    let path = "/tmp/golden/single-root.golden.sacm.xmi";
    assert_eq!(
        judge(file, &passing(path)),
        Ok(Verdict::Valid),
        "{ROW}: exit 0 is Valid"
    );
    let errors = vec![sacm_error(path, 4), sacm_error(path, 7)];
    assert_eq!(
        judge(file, &failing(path, &errors)),
        Ok(Verdict::Invalid(errors.clone())),
        "{ROW}: exit 3 is Invalid with every `Schemas validity error` line, in order"
    );
    let mut four = failing(path, &errors);
    four.status = Some(4);
    assert_eq!(
        judge(file, &four),
        Ok(Verdict::Invalid(errors)),
        "{ROW}: exit 4 is Invalid too"
    );

    let stderr = "warning: failed to load external entity \"x\"\nsecond line\n".to_owned();
    assert_eq!(
        judge(
            file,
            &Lint {
                status: Some(1),
                stderr: stderr.clone()
            }
        ),
        Err(format!(
            "xmllint failed on {file} (exit 1): warning: failed to load external entity \"x\""
        )),
        "{ROW}: exit 1 is an error naming the file, the status and the first stderr line"
    );
    match judge(
        file,
        &Lint {
            status: None,
            stderr: "killed\n".to_owned(),
        },
    ) {
        Err(message) => assert!(
            message.starts_with(&format!("xmllint failed on {file} (exit "))
                && message.ends_with("): killed"),
            "{ROW}: no status is an error naming the file and the first stderr line, got {message:?}"
        ),
        Ok(verdict) => panic!("{ROW}: no exit status is {verdict:?}, not an error"),
    }
    // Exit 3 or 4 with no validity error line judged nothing.
    for status in [3, 4] {
        assert_eq!(
            judge(
                file,
                &Lint {
                    status: Some(status),
                    stderr: stderr.clone()
                }
            ),
            Err(format!(
                "xmllint failed on {file} (exit {status}): warning: failed to load external entity \"x\""
            )),
            "{ROW}: exit {status} without a `Schemas validity error` line is an error naming the file, the status and the first stderr line"
        );
    }
    assert!(
        judge(
            file,
            &Lint {
                status: Some(5),
                stderr
            }
        )
        .is_err(),
        "{ROW}: any other status is an error"
    );
}

#[test]
fn lib_tool_023_a_metadata_only_file_must_fail_and_only_on_xmi_elements() {
    assert_eq!(
        METADATA_ONLY,
        [
            "xmi-metadata-valid.sacm.xmi",
            "xmi-metadata.golden.sacm.xmi"
        ],
        "{ROW}: the two listed fixtures"
    );
    let path = "/tmp/outputs/listed.xmi";
    let metadata = failing(path, &[xmi_error(path)]);
    let mixed = failing(path, &[xmi_error(path), sacm_error(path, 9)]);
    for listed in [
        "xmi-metadata.golden.sacm.xmi",
        "xmi-metadata-valid.sacm.xmi",
        // A strict output's name ends with the fixture's.
        "xmi__golden__xmi-metadata.golden.sacm.xmi",
        "xmi__xmi-metadata-valid.sacm.xmi",
    ] {
        assert_eq!(
            judge(listed, &passing(path)),
            Err(format!(
                "{listed} is listed in METADATA_ONLY but is valid; remove it from the list"
            )),
            "{ROW}: a listed file that is valid is an error, so a stale entry fails"
        );
        assert_eq!(
            judge(listed, &metadata),
            Ok(Verdict::MetadataOnly),
            "{ROW}: {listed}, failing on XMI-namespace elements only, is MetadataOnly"
        );
        assert_eq!(
            judge(listed, &mixed),
            Ok(Verdict::Invalid(vec![xmi_error(path), sacm_error(path, 9)])),
            "{ROW}: {listed}, failing on a SACM element too, is Invalid with every error"
        );
    }
    for unlisted in [
        "named.golden.sacm.xmi",
        "xmi__xmi-metadata-schema-valid.sacm.xmi",
    ] {
        assert_eq!(
            judge(unlisted, &metadata),
            Ok(Verdict::Invalid(vec![xmi_error(path)])),
            "{ROW}: {unlisted} is not listed, so an XMI-namespace error makes it Invalid"
        );
    }
    // A name that merely ends with a listed one is not listed: only the name
    // itself, or `__` and the name (a strict output's), is.
    assert_eq!(
        judge("foo-xmi-metadata-valid.sacm.xmi", &metadata),
        Ok(Verdict::Invalid(vec![xmi_error(path)])),
        "{ROW}: foo-xmi-metadata-valid.sacm.xmi is not listed, so an XMI-namespace error makes it Invalid"
    );
}

// ---------------------------------------------------------------------------
// 11 and 12: validate_with
// ---------------------------------------------------------------------------

/// A root with the golden files `goldens` and an out dir with the outputs
/// `outputs` (and a schema file), all with placeholder content.
fn validation_tree(test: &str, goldens: &[&str], outputs: &[&str]) -> (TempDir, PathBuf) {
    let root = TempDir::new(test);
    root.mkdir(GOLDEN_DIR);
    for name in goldens {
        root.write(&format!("{GOLDEN_DIR}/{name}"), b"<placeholder/>\n");
    }
    root.mkdir(&format!("out/{OUTPUTS_DIR}"));
    for name in outputs {
        root.write(&format!("out/{OUTPUTS_DIR}/{name}"), b"<placeholder/>\n");
    }
    root.write(&format!("out/{SCHEMA_FILE}"), b"<placeholder/>\n");
    let out_dir = root.path().join("out");
    (root, out_dir)
}

/// Runs `validate_with` with a fake lint that answers `answer(file name)`;
/// returns the result, the report and the (schema, file) of every call.
fn validate_fake(
    root: &Path,
    out_dir: &Path,
    answer: &dyn Fn(&str) -> Result<Lint, String>,
) -> (Result<(), String>, String, Vec<(PathBuf, PathBuf)>) {
    let calls = RefCell::new(Vec::new());
    let mut lint = |schema: &Path, file: &Path| {
        calls
            .borrow_mut()
            .push((schema.to_path_buf(), file.to_path_buf()));
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        answer(&name)
    };
    let mut out = Vec::new();
    let result = validate_with(root, out_dir, &mut lint, &mut out);
    let report =
        String::from_utf8(out).unwrap_or_else(|e| panic!("{ROW}: validate wrote non-UTF-8: {e}"));
    (result, report, calls.into_inner())
}

/// The lint calls `validate_with` must make: every golden file, then every
/// output, each in byte order of name, all with the generated schema.
fn expected_calls(
    root: &Path,
    out_dir: &Path,
    goldens: &[&str],
    outputs: &[&str],
) -> Vec<(PathBuf, PathBuf)> {
    let mut goldens = goldens.to_vec();
    goldens.sort_unstable();
    let mut outputs = outputs.to_vec();
    outputs.sort_unstable();
    let schema = out_dir.join(SCHEMA_FILE);
    goldens
        .iter()
        .map(|n| root.join(GOLDEN_DIR).join(n))
        .chain(outputs.iter().map(|n| out_dir.join(OUTPUTS_DIR).join(n)))
        .map(|file| (schema.clone(), file))
        .collect()
}

#[test]
fn lib_tool_023_validate_checks_goldens_then_outputs_in_name_order() {
    assert_eq!(
        (GOLDEN_DIR, OUTPUTS_DIR),
        ("crates/sacm/tests/data/sacm23/xmi/golden", "outputs"),
        "{ROW}: where the golden files and the outputs are"
    );
    let goldens = [
        "b.golden.sacm.xmi",
        "xmi-metadata.golden.sacm.xmi",
        "a.golden.sacm.xmi",
    ];
    let outputs = [
        "z.sacm.xmi",
        "Y.sacm.xmi",
        "xmi__xmi-metadata-valid.sacm.xmi",
        "m.sacm.xmi",
        "xmi__golden__xmi-metadata.golden.sacm.xmi",
    ];
    let (root, out_dir) = validation_tree("lib-tool-023-validate", &goldens, &outputs);
    // Not a golden file: not checked.
    root.write(&format!("{GOLDEN_DIR}/README.md"), b"not a golden file\n");
    let answer = |name: &str| {
        Ok(if name.contains("xmi-metadata") {
            failing(name, &[xmi_error(name)])
        } else {
            passing(name)
        })
    };
    let (result, report, calls) = validate_fake(root.path(), &out_dir, &answer);
    assert_eq!(
        result,
        Ok(()),
        "{ROW}: every file is valid or listed and metadata-only"
    );
    assert_eq!(
        report,
        "ok golden/a.golden.sacm.xmi\n\
         ok golden/b.golden.sacm.xmi\n\
         metadata golden/xmi-metadata.golden.sacm.xmi\n\
         ok outputs/Y.sacm.xmi\n\
         ok outputs/m.sacm.xmi\n\
         metadata outputs/xmi__golden__xmi-metadata.golden.sacm.xmi\n\
         metadata outputs/xmi__xmi-metadata-valid.sacm.xmi\n\
         ok outputs/z.sacm.xmi\n\
         schema: 3 golden files and 5 outputs checked: 5 valid, 3 invalid in XMI metadata only\n",
        "{ROW}: one line per file, goldens then outputs in name order, then the summary"
    );
    assert_eq!(
        calls,
        expected_calls(root.path(), &out_dir, &goldens, &outputs),
        "{ROW}: xmllint runs on every golden file, then every output, with the generated schema"
    );
}

/// Test 12: one invalid file is enough to fail the check.
fn assert_one_invalid_file_fails(goldens: &[&str]) {
    let outputs = ["p.sacm.xmi", "q.sacm.xmi"];
    let (root, out_dir) = validation_tree("lib-tool-023-one-invalid", goldens, &outputs);
    let answer = |name: &str| {
        Ok(if name == "p.sacm.xmi" {
            failing(name, &[sacm_error(name, 3)])
        } else {
            passing(name)
        })
    };
    let (result, _, _) = validate_fake(root.path(), &out_dir, &answer);
    assert_eq!(
        result,
        Err(format!(
            "1 of 4 files are not valid against the SACM XML Schema\noutputs/p.sacm.xmi: {}",
            sacm_error("p.sacm.xmi", 3)
        )),
        "{ROW}: a single invalid file fails the check"
    );
}

/// The count check's boundary (test 12): as many outputs as golden files
/// pass it, one fewer fails it.
fn assert_the_count_check_boundary() {
    let goldens = ["a.golden.sacm.xmi", "b.golden.sacm.xmi"];
    let (root, out_dir) = validation_tree(
        "lib-tool-023-equal",
        &goldens,
        &["x.sacm.xmi", "y.sacm.xmi"],
    );
    let (result, _, _) = validate_fake(root.path(), &out_dir, &|name: &str| Ok(passing(name)));
    assert_eq!(
        result,
        Ok(()),
        "{ROW}: as many outputs as golden files pass the count check"
    );
    let (root, out_dir) = validation_tree("lib-tool-023-one-fewer", &goldens, &["x.sacm.xmi"]);
    let (result, _, _) = validate_fake(root.path(), &out_dir, &|name: &str| Ok(passing(name)));
    assert_eq!(
        result,
        Err(format!(
            "fewer outputs (1) than golden files (2) in {}; run the strict-output step first",
            out_dir.join(OUTPUTS_DIR).display()
        )),
        "{ROW}: one output fewer than golden files fails the count check"
    );
}

#[test]
fn lib_tool_023_validate_fails_on_an_invalid_file_on_too_few_outputs_and_without_goldens() {
    // Invalid files: all problems in the error, every file still checked.
    let goldens = ["a.golden.sacm.xmi", "b.golden.sacm.xmi"];
    let outputs = ["p.sacm.xmi", "q.sacm.xmi", "r.sacm.xmi"];
    let (root, out_dir) = validation_tree("lib-tool-023-invalid", &goldens, &outputs);
    let answer = |name: &str| {
        Ok(match name {
            "b.golden.sacm.xmi" => failing(name, &[sacm_error(name, 4), sacm_error(name, 7)]),
            "q.sacm.xmi" => failing(name, &[sacm_error(name, 2)]),
            _ => passing(name),
        })
    };
    let (result, _, calls) = validate_fake(root.path(), &out_dir, &answer);
    let b = "b.golden.sacm.xmi";
    assert_eq!(
        result,
        Err([
            "2 of 5 files are not valid against the SACM XML Schema".to_owned(),
            format!("golden/{b}: {}", sacm_error(b, 4)),
            format!("golden/{b}: {}", sacm_error(b, 7)),
            format!("outputs/q.sacm.xmi: {}", sacm_error("q.sacm.xmi", 2)),
        ]
        .join("\n")),
        "{ROW}: the count, then one line per xmllint error, labelled with its file"
    );
    assert_eq!(
        calls,
        expected_calls(root.path(), &out_dir, &goldens, &outputs),
        "{ROW}: every file is checked"
    );

    assert_one_invalid_file_fails(&goldens);

    // A stale METADATA_ONLY entry fails, and every file is still checked.
    let outputs = [
        "p.sacm.xmi",
        "q.sacm.xmi",
        "xmi__xmi-metadata-valid.sacm.xmi",
    ];
    let (root, out_dir) = validation_tree("lib-tool-023-stale", &goldens, &outputs);
    let (result, _, calls) = validate_fake(root.path(), &out_dir, &|name: &str| Ok(passing(name)));
    let stale = "xmi__xmi-metadata-valid.sacm.xmi is listed in METADATA_ONLY but is valid; remove it from the list";
    assert!(
        result.as_ref().is_err_and(|e| e.contains(stale)),
        "{ROW}: a listed file that is valid fails the check, got {result:?}"
    );
    assert_eq!(
        calls,
        expected_calls(root.path(), &out_dir, &goldens, &outputs),
        "{ROW}: every file is checked"
    );

    // Too few outputs: the strict-output step did not run.
    let goldens = [
        "a.golden.sacm.xmi",
        "b.golden.sacm.xmi",
        "c.golden.sacm.xmi",
    ];
    let outputs = ["x.sacm.xmi", "y.sacm.xmi"];
    let (root, out_dir) = validation_tree("lib-tool-023-few", &goldens, &outputs);
    let (result, _, calls) = validate_fake(root.path(), &out_dir, &|name: &str| Ok(passing(name)));
    assert_eq!(
        result,
        Err(format!(
            "fewer outputs (2) than golden files (3) in {}; run the strict-output step first",
            out_dir.join(OUTPUTS_DIR).display()
        )),
        "{ROW}: fewer outputs than golden files fail, so the check cannot pass vacuously"
    );
    assert_eq!(
        calls,
        expected_calls(root.path(), &out_dir, &goldens, &outputs),
        "{ROW}: every file is checked"
    );

    // No golden files.
    let (root, out_dir) = validation_tree("lib-tool-023-no-goldens", &[], &["x.sacm.xmi"]);
    let (result, _, calls) = validate_fake(root.path(), &out_dir, &|name: &str| Ok(passing(name)));
    assert_eq!(
        result,
        Err(format!("no golden files in {GOLDEN_DIR}")),
        "{ROW}: no golden file fails the check"
    );
    assert_eq!(
        calls,
        expected_calls(root.path(), &out_dir, &[], &["x.sacm.xmi"]),
        "{ROW}: every file is checked"
    );

    // xmllint that cannot run fails the check with its error.
    let (root, out_dir) = validation_tree(
        "lib-tool-023-no-xmllint",
        &["a.golden.sacm.xmi"],
        &["x.sacm.xmi"],
    );
    let spawn = "could not run xmllint (is libxml2-utils installed?): not found";
    let (result, _, _) = validate_fake(root.path(), &out_dir, &|_: &str| Err(spawn.to_owned()));
    assert!(
        result.as_ref().is_err_and(|e| e.contains(spawn)),
        "{ROW}: a lint that cannot run fails the check with its error, got {result:?}"
    );

    assert_the_count_check_boundary();
}
