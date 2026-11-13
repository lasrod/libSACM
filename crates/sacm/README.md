# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3).

> **Status: early development.** The API is not stable yet. What is
> implemented and how it is tested is tracked requirement by requirement in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

## Status and plans

Implemented now: the document model with element names and each
Assertion's `assertionDeclaration`, the first value types (`LangString`,
`MultiLangString` and `AssertionDeclaration`) in `sacm::model`, and the
diagnostic type that findings are reported in (`sacm::diagnostic`), and
`sacm::validation::validate`, which reports each `ModelElement` with no name as a
`SACM-MULT-001` error. `xmi::read`'s refusals carry catalog codes. `sacm::xmi::read` reads
`AssuranceCasePackage` and `ArgumentPackage` roots, the AssuranceCasePackages,
ArgumentPackages and Claims in them, their names and each Claim's `assertionDeclaration`;
it refuses what it cannot represent rather than drop it.
`sacm::xmi::write` writes a document, its names and its declarations deterministically;
in strict mode, the default, it refuses a document that validation reports an Error for.
`sacm::edit::apply` creates AssuranceCasePackages, ArgumentPackages and Claims, each with
a name and a given or generated id; a refused operation changes nothing.
`sacm::compare::semantic_compare` compares two documents by SACM content and names each
difference.

Planned:

- A typed model of the full SACM 2.3 metamodel: assurance case, argumentation,
  artifact and terminology packages, with their interfaces and bindings.
- XMI import and deterministic export. Strict mode writes only
  schema-conforming, valid SACM 2.3 models; tolerant mode reads third-party
  dialects and legacy revisions and tells you exactly what it accepted.
- More validation rules against the specification's constraints, reported as structured
  diagnostics with stable codes, severities, clause references and locations.
- SACM-native editing for every element kind. Every public mutation either succeeds,
  keeping the document's structural invariants, or fails and leaves it unchanged.
  Destructive edits can be previewed first.

## Usage

```toml
[dependencies]
sacm = "0.1"
```

```rust
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};
use sacm::{diagnostic, validation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
         xmlns:sacm="http://www.omg.org/spec/SACM/20220301">
  <sacm:AssuranceCasePackage xmi:id="case"/>
  <sacm:ArgumentPackage xmi:id="arguments">
    <name lang="en" content="Arguments"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
      <name lang="en" content="The system is safe"/>
    </argumentElement>
  </sacm:ArgumentPackage>
</xmi:XMI>"#;
    let output = xmi::read(input, &ReadOptions::default())?;
    let document = output.document();
    for element in document.elements() {
        let name = element.name().and_then(|n| n.content());
        println!("{} {} {name:?}", element.kind(), element.id());
    }

    // Check it against SACM's rules: the case package has no name.
    let mut findings = output.diagnostics().to_vec();
    findings.extend(validation::validate(document));
    findings.sort();
    for finding in &findings {
        println!("{finding}");
    }
    assert!(diagnostic::has_errors(&findings));

    // A strict write, the default, refuses it; a tolerant one writes it as it is held.
    let refused = xmi::write(document, &WriteOptions::default());
    assert!(matches!(refused, Err(WriteError::Invalid { .. })));
    let text = xmi::write(document, &WriteOptions::default().with_mode(Mode::Tolerant))?;
    print!("{text}");
    Ok(())
}
```

`sacm::xmi::read` reads `AssuranceCasePackage` and `ArgumentPackage` roots, the
AssuranceCasePackages, ArgumentPackages and Claims in them, their names and each Claim's
`assertionDeclaration`. It refuses everything else (other kinds, a `description`, a
`gid`, an `assertionDeclaration` value that is not a SACM literal) as
`ReadError::Unsupported` rather than drop it, and reports an `xsi:type` (SACM-XMI-010)
or an `xmi:version` (SACM-XMI-011) as a diagnostic. `sacm::xmi::write` writes the
document back deterministically. By default it is strict: it refuses a document that
validation reports an Error for as `WriteError::Invalid`. `Mode::Tolerant` writes the
document as it is held.

Build a document with `sacm::edit::apply`, then validate, write and read it back:

```rust
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
use sacm::model::{Document, LangString};
use sacm::validation;
use sacm::xmi::{self, ReadOptions, WriteOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = |text: &str| LangString::new(text).with_lang("en");
    let mut doc = Document::new();

    edit::apply(&mut doc, CreateAssuranceCasePackage::root(name("Safety case")).with_id("case"))?;
    // No with_id: the id is generated. Use it as the next parent.
    let created = edit::apply(&mut doc, CreateArgumentPackage::new("case", name("Arguments")))?;
    let arguments = created.created_ids()[0].clone();
    assert_eq!(arguments.as_str(), "argument_package_1");
    edit::apply(&mut doc, CreateClaim::new(&arguments, name("The system is safe")).with_id("c_top"))?;

    // Every element has a name, so validation finds nothing.
    assert!(validation::validate(&doc).is_empty());
    let text = xmi::write(&doc, &WriteOptions::default())?;
    assert!(text.contains(r#"xmi:id="c_top""#));
    print!("{text}");

    // And it reads back.
    let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
    let parent = back.get("c_top").and_then(|claim| claim.parent());
    assert_eq!(parent.map(|parent| parent.id().as_str()), Some("argument_package_1"));
    Ok(())
}
```

`sacm::xmi::read` reads such a file back with the same ids, kinds, names, parents and
order.

See the [crate documentation](https://docs.rs/sacm) for the full API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
