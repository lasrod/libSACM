# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3).

> **Status: early development.** The API is not stable yet. What is
> implemented and how it is tested is tracked requirement by requirement in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

## What works today

- `sacm::xmi::read` reads `AssuranceCasePackage` and `ArgumentPackage` roots and the
  packages and Claims in them, with their names and each Claim's `assertionDeclaration`.
  It refuses what it cannot represent yet rather than drop it.
- `sacm::xmi::write` writes a document deterministically. In strict mode, the default, it
  refuses a document that validation reports an Error for.
- `sacm::validation::validate` reports an element with no name and an id that is not an
  XML NCName, as diagnostics with stable codes.
- `sacm::edit::apply` creates AssuranceCasePackages, ArgumentPackages and Claims; a
  refused edit changes nothing.
- `sacm::compare::semantic_compare` lists every difference in SACM content between two
  documents.

The [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md)
tracks every requirement: what is built, what is planned, and the tests behind each.

Planned:

- A typed model of the full SACM 2.3 metamodel: assurance case, argumentation,
  artifact and terminology packages, with their interfaces and bindings.
- Tolerant reading of third-party dialects and legacy revisions, reporting exactly what
  was accepted.
- More validation rules against the specification's constraints.
- SACM-native editing for every element kind, with previews of destructive edits.

## Usage

```toml
[dependencies]
sacm = "0.1"
```

Load a document, inspect it, edit it, validate it and save it:

```rust
use sacm::diagnostic;
use sacm::edit::{self, CreateClaim};
use sacm::model::{ElementKind, LangString};
use sacm::validation;
use sacm::xmi::{self, ReadOptions, WriteOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load. The input is the bytes of an XMI file; here it is inline.
    let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name lang="en" content="Braking case"/>
  <argumentPackage xmi:id="arguments">
    <name lang="en" content="Arguments"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
      <name lang="en" content="The brake stops the vehicle"/>
    </argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>"#;
    let output = xmi::read(input, &ReadOptions::default())?;
    assert!(output.diagnostics().is_empty());
    let mut document = output.into_document();

    // Inspect.
    let c1 = document.get("c1").expect("the Claim was read");
    assert_eq!(c1.kind(), ElementKind::Claim);
    assert_eq!(c1.name().and_then(|name| name.content()), Some("The brake stops the vehicle"));
    assert_eq!(c1.parent().map(|parent| parent.id().as_str()), Some("arguments"));

    // Edit: add a Claim to the ArgumentPackage. Its id is generated.
    let name = LangString::new("The brake is tested").with_lang("en");
    let created = edit::apply(&mut document, CreateClaim::new("arguments", name))?;
    assert_eq!(created.created_ids()[0].as_str(), "claim_1");

    // Validate: every element has a name and a valid id.
    let findings = validation::validate(&document);
    assert!(!diagnostic::has_errors(&findings));

    // Save. The write is strict by default; writing the file is ordinary I/O,
    // such as std::fs::write("case.sacm.xmi", &text).
    let text = xmi::write(&document, &WriteOptions::default())?;

    // The saved text reads back as the same document.
    let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
    assert_eq!(back, document);
    Ok(())
}
```

A strict save refuses a document with a validation Error, as `WriteError::Invalid`; a
draft is saved with `WriteOptions::default().with_mode(Mode::Tolerant)`.

See the [crate documentation](https://docs.rs/sacm) for the full API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
