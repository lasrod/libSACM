# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3).

> **Status: early development.** The API is not stable yet. What is
> implemented and how it is tested is tracked requirement by requirement in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

## Status and plans

Implemented now: the read-only document model with element names and each
Assertion's `assertionDeclaration`, the first value types (`LangString`,
`MultiLangString` and `AssertionDeclaration`) in `sacm::model`, and the
diagnostic type that findings are reported in (`sacm::diagnostic`), and
`sacm::validation::validate`, which reports each `ModelElement` with no name as a
`SACM-MULT-001` error. `xmi::read`'s refusals carry catalog codes. `sacm::xmi::read` reads
`AssuranceCasePackage` and `ArgumentPackage` roots and their names, but nothing else
inside them yet; it refuses what it cannot represent rather than drop it.
`sacm::xmi::write` writes a document and its names deterministically, and refuses a
non-default `assertionDeclaration`, which it does not write yet.

Planned:

- A typed model of the full SACM 2.3 metamodel: assurance case, argumentation,
  artifact and terminology packages, with their interfaces and bindings.
- XMI import and deterministic export. Strict mode writes only
  schema-conforming, valid SACM 2.3 models; tolerant mode reads third-party
  dialects and legacy revisions and tells you exactly what it accepted.
- More validation rules against the specification's constraints, reported as structured
  diagnostics with stable codes, severities, clause references and locations.
- SACM-native editing. Every public mutation either succeeds, keeping the
  document's structural invariants, or fails and leaves it unchanged.
  Destructive edits can be previewed first.

## Usage

```toml
[dependencies]
sacm = "0.1"
```

```rust
use sacm::xmi::{self, ReadOptions, WriteOptions};
use sacm::{diagnostic, validation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
         xmlns:sacm="http://www.omg.org/spec/SACM/20220301">
  <sacm:AssuranceCasePackage xmi:id="case"/>
  <sacm:ArgumentPackage xmi:id="arguments">
    <name lang="en" content="Arguments"/>
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

    let text = xmi::write(document, &WriteOptions::default())?;
    print!("{text}");
    Ok(())
}
```

`sacm::xmi::read` reads `AssuranceCasePackage` and `ArgumentPackage` roots and their names
so far; it refuses the other content of packages rather than drop it. `sacm::xmi::write`
writes the document back deterministically, and refuses a non-default
`assertionDeclaration` until a later release writes it.

See the [crate documentation](https://docs.rs/sacm) for the full API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
