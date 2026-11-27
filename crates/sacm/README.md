# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3),
the OMG standard for exchanging assurance cases between tools as XMI files.

> **Status: pre-1.0.** The API is not stable yet. The
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md)
> shows what is verified, requirement by requirement.

## What it does

| To | Start at |
|---|---|
| Read an XMI file | `sacm::xmi::read` |
| Inspect elements | `Document::get`, `SacmElement` |
| Check the model against the SACM 2.3 rules | `sacm::validation::validate` |
| Change the model | `sacm::edit::apply` |
| Compare two documents | `sacm::compare::semantic_compare` |
| Write an XMI file | `sacm::xmi::write` |

- **All four package kinds.** `AssuranceCasePackage`, `ArgumentPackage`, `ArtifactPackage`
  and `TerminologyPackage`, with their elements, interfaces and bindings.
- **Four compliance points claimed.** Assurance Case, Argumentation, Artifact and
  Terminology (SACM 2.3 clause 2). The
  [conformance statement](https://github.com/lasrod/libSACM/blob/main/docs/sacm/compliance-points.md)
  says what that covers and where it stops.
- **Nothing dropped.** Content that is not SACM 2.3 is kept as written, with a diagnostic.
- **Atomic edits.** A refused operation changes nothing.
- **Safe on input.** No `unsafe`, no panics on input, bounded reads.

Planned for 1.0: a user guide, a SACM primer, runnable examples, property-based tests,
fuzzing and semver checks.

## Usage

```toml
[dependencies]
sacm = "0.4"
```

Read a document, inspect it, edit it, validate it and write it:

```rust
use sacm::edit::{self, CreateClaim};
use sacm::model::ElementKind;
use sacm::xmi::{self, ReadOptions, WriteOptions};
use sacm::{diagnostic, validation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read. The input is the bytes of an XMI file, such as
    // `std::fs::read("case.sacm.xmi")?`; here it is inline.
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
    let created = edit::apply(&mut document, CreateClaim::new("arguments", "The brake is tested"))?;
    assert_eq!(created.created_ids(), ["claim_1"]);

    // Validate: no finding is an Error.
    let findings = validation::validate(&document);
    assert!(!diagnostic::has_errors(&findings));

    // Write, in strict mode by default. Saving the text is ordinary I/O, such
    // as `std::fs::write("case.sacm.xmi", &text)?`.
    let text = xmi::write(&document, &WriteOptions::default())?;

    // The text reads back as the same document.
    let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
    assert_eq!(back, document);
    Ok(())
}
```

## Strict and tolerant modes

Reading and writing are strict by default.

- **Open another tool's file:** read with `with_mode(Mode::Tolerant)`. It keeps what a
  strict read refuses, as preserved content with a Warning.
- **Hand a file on:** write in strict mode. It refuses output that would not be
  schema-valid, and a document that holds preserved content.
- **Check the model:** call `validation::validate`. A strict write never validates, so a
  draft with a model Error saves.

The [crate documentation](https://docs.rs/sacm) has the rules in full, the `serde` feature
and the API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
