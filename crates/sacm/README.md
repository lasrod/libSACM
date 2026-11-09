# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3).

> **Status: early development.** The API is not stable yet. What is
> implemented and how it is tested is tracked requirement by requirement in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

## Planned

Implemented now: the read-only document model (`sacm::model`) and the
diagnostic type that findings are reported in (`sacm::diagnostic`). Nothing
emits diagnostics yet.

- A typed model of the full SACM 2.3 metamodel: assurance case, argumentation,
  artifact and terminology packages, with their interfaces and bindings.
- XMI import and deterministic export. Strict mode writes conformant SACM 2.3;
  tolerant mode reads third-party dialects and legacy revisions and tells you
  exactly what it accepted.
- Validation against the specification's constraints, reported as structured
  diagnostics with stable codes, severities, clause references and locations.
- SACM-native editing. Every public mutation either succeeds and leaves the
  document valid, or fails and leaves it unchanged. Destructive edits can be
  previewed first.

## Usage

```toml
[dependencies]
sacm = "0.1"
```

```rust
use sacm::model::{AbstractClass, Document, ElementKind};

/// Prints every element, parents before children.
fn print_elements(doc: &Document) {
    for element in doc.elements() {
        println!("{} {}", element.kind(), element.id());
    }
}

fn main() {
    assert!(ElementKind::Claim.is_a(AbstractClass::Assertion));
    print_elements(&Document::new());
}
```

There is no stable way to fill a `Document` yet: the XMI reader comes next.

See the [crate documentation](https://docs.rs/sacm) for the full API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
