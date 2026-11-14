# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3).

> **Status: early development.** The API is not stable yet. What is
> implemented and how it is tested is tracked requirement by requirement in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

## What works today

- `sacm::xmi::read` reads `AssuranceCasePackage` and `ArgumentPackage` roots and the
  packages and Claims in them, with their names, `gid`s and each Claim's
  `assertionDeclaration`, and the descriptions, notes, implementation constraints and
  tagged values of each (`SacmElement::descriptions`, `notes`, `tagged_values` and the
  like), with their `content` and, on a tagged value, `key` (`SacmElement::content`,
  `SacmElement::key`). Every element's `isCitation`, `citedElement`, `isAbstract` and
  `abstractForm` are read too (`SacmElement::is_citation`, `cited_element` and the like),
  in the attribute or the element form; a reference is held as the id it names, and one
  that names no element is kept. It refuses what it cannot represent yet rather than drop it. An
  element without an `xmi:id` gets a generated one, such as `claim_1`, which `write`
  saves; see the Ids section of the `sacm::xmi::read` docs.
- `sacm::xmi::write` writes a document deterministically, and by default refuses one that
  validation reports an Error for.
- `sacm::validation::validate` reports each element that has no name, each `content` or
  `key` text that is empty or gives one language twice, each tagged value without a key,
  each id that is not an XML `NCName`, each `gid` two elements share, each element with
  more than one description, each `ArgumentPackage` that nests one and also holds
  other argumentation elements, each reference that names no element, each element with a
  `citedElement` whose `isCitation` is not true, and each `abstractForm` that breaks a
  §8.2 rule, as diagnostics
  with stable codes.
- `sacm::edit::apply` creates AssuranceCasePackages, ArgumentPackages and Claims, and
  descriptions, implementation constraints, notes and tagged values on them
  (`sacm::edit::CreateNote`, `sacm::edit::CreateTaggedValue` and the like); a refused
  edit changes nothing.
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
    // Load. The input is the bytes of an XMI file, such as
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
    let name = LangString::new("The brake is tested").with_lang("en");
    let created = edit::apply(&mut document, CreateClaim::new("arguments", name))?;
    assert_eq!(created.created_ids()[0].as_str(), "claim_1");

    // Validate. This release checks names, ids, gids, descriptions, the content and key
    // of descriptions, implementation constraints, notes and tagged values, what an
    // ArgumentPackage holds, and references, citations and abstract forms.
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
draft is saved with `WriteOptions::default().with_mode(Mode::Tolerant)`. A later release
may add validation rules, so a strict save may then refuse a document it saved before.

Which `xmi::Mode` to use: read in tolerant mode, the default, to open files from other
tools. A strict read reports the variations it recognises as Errors instead of Warnings
(in this release, only `xsi:type`). Neither mode validates what it reads, so to check a
file, also run `validation::validate` and test both lists with `diagnostic::has_errors`.
Save in strict mode, the default, for a file you hand on.

See the [crate documentation](https://docs.rs/sacm) for the full API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
