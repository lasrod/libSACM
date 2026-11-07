# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3).

> **Status: early development.** The API is not stable yet. What is
> implemented and how it is tested is tracked requirement by requirement in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

## What it does

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
println!("sacm {} implements SACM {}", sacm::VERSION, sacm::SACM_VERSION);
```

See the [crate documentation](https://docs.rs/sacm) for the full API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
