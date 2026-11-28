# User guide

How to read, inspect, validate, edit and save a SACM 2.3 assurance case with the `sacm` crate.

- **New to SACM?** The [SACM primer](https://github.com/lasrod/libSACM/blob/main/crates/sacm/PRIMER.md)
  explains packages, claims, asserted relationships and the rest.
- **Every Rust code block runs.** Each is compiled and run as a test of the crate, so it
  works as shown.
- **One type is the centre.** A `Document` holds one file's elements: read it, work on
  it, write it.

| Step | Start at |
|---|---|
| [Read a file](#read-a-file) | `xmi::read` |
| [Inspect the model](#inspect-the-model) | `Document::get`, `SacmElement` |
| [Validate and read diagnostics](#validate-and-read-diagnostics) | `validation::validate` |
| [Edit](#edit) | `edit::apply` |
| [Save](#save) | `xmi::write` |
| [Strict and tolerant modes](#strict-and-tolerant-modes) | `xmi::Mode` |

## Read a file

`xmi::read` takes the bytes of an XMI file and gives a `ReadOutput`: the document and
the diagnostics of the read.

```rust
use sacm::xmi::{self, ReadOptions};

// The bytes of a file, such as `std::fs::read("case.sacm.xmi")?`; here inline.
let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name lang="en" content="Blender case"/>
  <argumentPackage xmi:id="arguments">
    <name lang="en" content="Arguments"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
      <name lang="en" content="The blade is guarded"/>
    </argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>"#;

let output = xmi::read(input, &ReadOptions::default())?;
assert!(output.diagnostics().is_empty());

let document = output.into_document();
assert_eq!(document.element_count(), 3);
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **Strict by default.** `ReadOptions::default()` reads in strict mode;
  [Strict and tolerant modes](#strict-and-tolerant-modes) shows the other.
- **Two kinds of trouble.** Input that cannot be read at all is an `Err(ReadError)`,
  such as malformed XML. A finding about content that was read is a diagnostic.
- **Check before you go on.** `ReadOutput::has_errors` says whether any diagnostic of
  the read is an Error.
- **Other files are never opened.** A reference into another file is kept as written.

## Inspect the model

`Document::get` finds an element by its id and gives a `SacmElement`, a view that
borrows the document. A reference is the id it names; `get` resolves it.

```rust
use sacm::model::{ElementKind, ReferenceEnd};
use sacm::xmi::{self, ReadOptions};

let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
  <name lang="en" content="Arguments"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
    <name lang="en" content="The blade is guarded"/>
    <description xmi:id="c1_d1">
      <content>
        <value lang="en" content="The lid covers the blade"/>
      </content>
    </description>
  </argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c2">
    <name lang="en" content="The motor stops when the lid opens"/>
  </argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2" target="c1">
    <name lang="en" content="Guarding by interlock"/>
  </argumentElement>
</sacm:ArgumentPackage>"#;
let document = xmi::read(input, &ReadOptions::default())?.into_document();

// One element: its kind, its name and where it sits.
let c1 = document.get("c1").expect("the file has c1");
assert_eq!(c1.kind(), ElementKind::Claim);
assert_eq!(c1.name().and_then(|name| name.content()), Some("The blade is guarded"));
assert_eq!(c1.parent().map(|parent| parent.id().as_str()), Some("arguments"));

// Its description: an element of its own, whose content has one value per language.
let description = c1.descriptions().next().expect("c1 has a description");
let content = description.content().expect("the description has a content");
assert_eq!(content.values()[0].lang(), Some("en"));
assert_eq!(content.values()[0].content(), Some("The lid covers the blade"));

// A relationship holds ids. `get` resolves one.
let i1 = document.get("i1").expect("the file has i1");
assert_eq!(i1.sources().len(), 1);
let source = document.get(&i1.sources()[0]).expect("the source is in the file");
assert_eq!(source.id().as_str(), "c2");
assert_eq!(i1.target().map(|target| target.as_str()), Some("c1"));

// The other way round: what names c1, and at which end.
let referrers = document.referrers("c1");
assert_eq!(referrers.len(), 1);
assert_eq!(referrers[0].holder().id().as_str(), "i1");
assert_eq!(referrers[0].end(), ReferenceEnd::Target);

// Walk the tree: the roots, then everything under one, with its depth.
let root = document.roots().next().expect("the file has a root");
let below: Vec<(&str, usize)> =
    root.descendants().map(|(element, depth)| (element.id().as_str(), depth)).collect();
assert_eq!(below, [("c1", 1), ("c1_d1", 2), ("c2", 1), ("i1", 1)]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **A name may be missing.** `name()` gives `None` for an element read without one.
- **A name is one value; a content is several.** A name is a `LangString`. The content
  of a description, note or tagged value is a `MultiLangString`.
- **Tagged values by key.** `tagged_value("status")` finds an element's tagged value with
  that key; [Edit one language's value](#edit-one-languages-value) uses it.

## Validate and read diagnostics

`validation::validate` checks a document against the rules of SACM 2.3 and gives its
findings as diagnostics. Neither `read` nor `write` runs it; call it yourself.

```rust
use sacm::diagnostic::{self, Code, Severity};
use sacm::model::ElementId;
use sacm::validation;
use sacm::xmi::{self, ReadOptions};

// The source `c9` of the relationship names no element.
let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
  <name lang="en" content="Arguments"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
    <name lang="en" content="The blade is guarded"/>
  </argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c9" target="c1">
    <name lang="en" content="Guarding by interlock"/>
  </argumentElement>
</sacm:ArgumentPackage>"#;

// The file is well-formed SACM 2.3, so the read itself reports nothing.
let output = xmi::read(input, &ReadOptions::default())?;
assert!(!output.has_errors());
let document = output.into_document();

let findings = validation::validate(&document);
assert!(diagnostic::has_errors(&findings));
assert_eq!(findings.len(), 1);

let finding = &findings[0];
assert_eq!(finding.code(), Code::REF_DANGLING);
assert_eq!(finding.code().as_str(), "SACM-REF-001");
assert_eq!(finding.severity(), Severity::Error);
assert_eq!(finding.affected(), [ElementId::from("i1"), ElementId::from("c9")]);
assert_eq!(finding.clause(), Some("11.13"));

// The message is English text for a person. Do not match on it.
println!("{}", finding.message());
# Ok::<(), Box<dyn std::error::Error>>(())
```

What a diagnostic holds:

| Part | Read with | Use it to |
|---|---|---|
| Code | `code()` | Decide what to do. A code is stable, such as SACM-REF-001. |
| Severity | `severity()` | Tell an Error, which does not conform, from a Warning. |
| Affected elements | `affected()` | Show the user where: the ids of the elements concerned. |
| Clause | `clause()` | Point at the clause of the standard, when there is one. |
| Location | `location()` | Show the line and column in the file, for a diagnostic of a read. |
| Message | `message()` | Show a person what is wrong, in English. |

- **To check that a file conforms,** read it in strict mode, run `validate`, and test
  both results: `ReadOutput::has_errors` and `diagnostic::has_errors`.
- **Every code** is in the
  [diagnostics catalog](https://github.com/lasrod/libSACM/blob/main/docs/sacm/diagnostics.md),
  with what it means.
- **Other languages.** A diagnostic also gives its message as a template and arguments;
  the `translate_diagnostics` [example](#examples) shows how to use them.

## Edit

Every change is an operation, a plain value, given to `edit::apply`. It returns a
`Mutation`: the ids the operation created, changed and deleted.

```rust
use sacm::edit::{self, CreateAssertedInference, CreateClaim, DeleteElement, SetName};
use sacm::model::ElementId;
use sacm::xmi::{self, ReadOptions};

let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
  <name lang="en" content="Arguments"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
    <name lang="en" content="The blade is guarded"/>
  </argumentElement>
</sacm:ArgumentPackage>"#;
let mut document = xmi::read(input, &ReadOptions::default())?.into_document();

// Create a Claim in the package. Without `with_id`, its id is generated.
let created = edit::apply(&mut document, CreateClaim::new("arguments", "The lid locks"))?;
assert_eq!(created.created_ids(), [ElementId::from("claim_1")]);

// Create a relationship: the new Claim supports c1. Here the id is given.
let inference = CreateAssertedInference::new("arguments", "Guarding", ["claim_1"], "c1");
edit::apply(&mut document, inference.with_id("i1"))?;

// Change a feature of an element, named by its id.
let renamed = edit::apply(&mut document, SetName::new("claim_1", "The lid locks shut"))?;
assert_eq!(renamed.changed_ids(), [ElementId::from("claim_1")]);

// Delete an element that nothing references.
let deleted = edit::apply(&mut document, DeleteElement::new("i1"))?;
assert_eq!(deleted.deleted_ids(), [ElementId::from("i1")]);
assert_eq!(document.element_count(), 3);
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **Atomic.** An operation is applied whole or refused whole.
- **One operation per kind of change.** There is a create for each kind of element, a
  set for each feature, and `DeleteElement`, `MoveElement` and `CopyElements`.
- **Several as one.** `edit::batch` and `edit::apply_all` apply several operations as
  one change: all of them, or none.
- **Content rules are not refused.** An edit accepts an AssertedEvidence whose source is a
  Claim, say; `validate` reports it (SACM-REF-002, clause 11.15).

### Edit one language's value

A set replaces a whole feature. To change one language's value and keep the rest, take
a copy of what the element holds, change the one value, and set the copy.

```rust
use sacm::edit::{self, SetContent, SetName, SetTaggedValue};
use sacm::model::ElementId;
use sacm::xmi::{self, ReadOptions};

let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name lang="en" content="Blender case"/>
  <terminologyPackage xmi:id="terms">
    <name lang="en" content="Terms"/>
    <terminologyElement xmi:type="sacm:Term" xmi:id="blade" value="blade">
      <name lang="en" content="Blade"/>
    </terminologyElement>
    <terminologyElement xmi:type="sacm:Term" xmi:id="lid" value="lid">
      <name lang="en" content="Lid"/>
    </terminologyElement>
  </terminologyPackage>
  <argumentPackage xmi:id="arguments">
    <name lang="en" content="Arguments"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
      <name lang="en" content="The blade is guarded"/>
      <description xmi:id="c1_d1">
        <content>
          <value lang="en" content="The lid covers the blade"/>
          <value lang="sv" content="Locket skyddar bladet"/>
        </content>
      </description>
      <taggedValue xmi:id="c1_tv1">
        <content>
          <value lang="en" content="draft"/>
          <value lang="sv" content="utkast"/>
        </content>
        <key>
          <value content="status"/>
        </key>
      </taggedValue>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2">
      <name lang="en" content="The lid is shut"/>
      <description xmi:id="c2_d1">
        <content>
          <value xmi:type="sacm:ExpressionLangString" lang="en" expression="blade"/>
        </content>
      </description>
    </argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>"#;
let mut document = xmi::read(input, &ReadOptions::default())?.into_document();
assert!(sacm::validation::validate(&document).is_empty());

// A name is one value: change its content, and keep its language.
let c1 = document.get("c1").expect("the file has c1");
let mut name = c1.name().expect("c1 has a name").clone();
name.set_content(Some("The blade is covered".to_owned()));
edit::apply(&mut document, SetName::new("c1", name))?;

// A description: change the Swedish value only.
let description = document.get("c1_d1").expect("the file has c1_d1");
let mut content = description.content().expect("it has a content").clone();
let sv = content.index_for_lang("sv").expect("it has a value to show for Swedish");
content.values_mut()[sv].set_content(Some("Locket täcker bladet".to_owned()));
let changed = edit::apply(&mut document, SetContent::new("c1_d1", content))?;
assert_eq!(changed.changed_ids(), [ElementId::from("c1_d1")]);

// A tagged value, found by its key: change the English value only.
let c1 = document.get("c1").expect("the file has c1");
let tag = c1.tagged_value("status").expect("c1 has the tagged value");
let mut status = tag.content().expect("it has a content").clone();
let en = status.index_for_lang("en").expect("it has a value to show for English");
status.values_mut()[en].set_content(Some("reviewed".to_owned()));
edit::apply(&mut document, SetTaggedValue::new("c1", "status", status))?;

// An expression value links a Term or an Expression. Link another: the value keeps
// its language and its place.
let linked = document.get("c2_d1").expect("the file has c2_d1");
let mut content = linked.content().expect("it has a content").clone();
content.values_mut()[0] = content.values()[0].clone().with_expression("lid");
edit::apply(&mut document, SetContent::new("c2_d1", content))?;

// Each value changed; its neighbours did not.
let c1 = document.get("c1").expect("the file has c1");
assert_eq!(c1.name().and_then(|name| name.lang()), Some("en"));
let description = document.get("c1_d1").and_then(|d1| d1.content()).expect("as before");
assert_eq!(description.values()[0].content(), Some("The lid covers the blade"));
assert_eq!(description.values()[1].content(), Some("Locket täcker bladet"));
let status = c1.tagged_value("status").and_then(|tag| tag.content()).expect("as before");
assert_eq!(status.values()[0].content(), Some("reviewed"));
assert_eq!(status.values()[1].content(), Some("utkast"));
let linked = document.get("c2_d1").and_then(|d1| d1.content()).expect("as before");
assert_eq!(linked.values()[0].expression(), Some(&ElementId::from("lid")));
assert_eq!(linked.values()[0].lang(), Some("en"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **`index_for_lang` picks the value to show.** It takes the same language, then the same
  primary subtag (`en-GB` for `en-US`), then the first value with text. To change exactly
  one language, check the `lang()` of the value it found.
- **`values_mut` keeps the number of values.** To add a language, `push` a value.
- **An expression value** is an `ExpressionLangString`. A set checks a link the element
  did not hold before: it must name a `Term` or an `Expression` of the document.
- **`SetTaggedValue` creates** the tagged value if the element has none with the key.

### A refused edit

A refused operation is an `Err(EditError)`, and the document is as it was. The error
says what was refused; its `code()` is stable.

```rust
use sacm::diagnostic::Code;
use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, SetName};
use sacm::model::Document;

// A document can also be built in code.
let mut document = Document::new();
edit::apply(&mut document, CreateArgumentPackage::root("Arguments").with_id("arguments"))?;
let before = document.clone();

// The id `argumens` is misspelt: the document has no such element.
let refused = edit::apply(&mut document, CreateClaim::new("argumens", "The lid locks"));
let error = refused.expect_err("the parent is missing");

// Show the user the error: it prints what was refused.
assert_eq!(
    error.to_string(),
    r#"the document has no element "argumens" to be the parent"#
);
// Decide in code by the variant or the code, never by the text.
assert!(matches!(error, EditError::ParentNotFound { .. }));
assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND);

// Nothing changed.
assert_eq!(document, before);

// To ask first, without changing anything, use `check`.
let operation = SetName::new("c9", "Renamed").into();
assert!(edit::check(&document, &operation).is_err());
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **Handle it like any error.** Show the message, let the user correct the input, and
  apply again. There is nothing to roll back.
- **`check` and `preview`.** `edit::check` says whether an operation would be accepted;
  `edit::preview` also says what it would create, change and delete.
- **A delete refuses by default** when the element is referenced or contains elements.
  Its policies say what may be deleted with it.

## Save

`xmi::write` gives the document as XMI text. Saving the text is ordinary I/O.

```rust
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
use sacm::model::{Document, LangString};
use sacm::xmi::{self, ReadOptions, WriteOptions};

let name = |text: &str| LangString::new(text).with_lang("en");
let mut document = Document::new();
edit::apply(&mut document, CreateAssuranceCasePackage::root(name("Blender case")).with_id("case"))?;
edit::apply(&mut document, CreateArgumentPackage::new("case", name("Arguments")).with_id("arguments"))?;
edit::apply(&mut document, CreateClaim::new("arguments", name("The blade is guarded")).with_id("c1"))?;

// Strict mode by default. Then, say, `std::fs::write("case.sacm.xmi", &text)?`.
let text = xmi::write(&document, &WriteOptions::default())?;
assert!(text.contains(r#"<argumentElement xmi:type="sacm:Claim" xmi:id="c1">"#));

// The text reads back as the same document.
let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
assert_eq!(back, document);
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **Deterministic.** The same document gives the same text.
- **A write never validates.** A draft with a model Error, such as a dangling reference,
  still saves. Run `validate` when you need to know.
- **A refusal is an `Err(WriteError)`,** and nothing is written. The next section shows one.

## Strict and tolerant modes

Reading and writing each take a mode, strict by default. Neither mode drops model
content silently.

| | Strict (the default) | Tolerant |
|---|---|---|
| Read | Refuses, or reports as an Error, what is not SACM 2.3. | Opens files from other tools and keeps most of what strict refuses, with a Warning. |
| Write | Refuses output that would not be schema-valid. | Writes back what a read kept. |

Content that is not SACM 2.3 where it stands is kept exactly as written, as preserved
content of its element. Here another tool has added an attribute of its own:

```rust
use sacm::diagnostic::{Code, Severity};
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
    xmlns:tool="http://example.org/tool" xmi:id="arguments">
  <name lang="en" content="Arguments"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1" tool:colour="green">
    <name lang="en" content="The blade is guarded"/>
  </argumentElement>
</sacm:ArgumentPackage>"#;

// A strict read reports the attribute as an Error: the file does not conform.
let strict = xmi::read(input, &ReadOptions::default())?;
assert!(strict.has_errors());

// A tolerant read reports the same finding as a Warning.
let tolerant = xmi::read(input, &ReadOptions::default().with_mode(Mode::Tolerant))?;
assert!(!tolerant.has_errors());
let finding = &tolerant.diagnostics()[0];
assert_eq!(finding.code(), Code::XMI_UNKNOWN_CONTENT);
assert_eq!(finding.severity(), Severity::Warning);
assert_eq!(finding.location().map(|at| at.line()), Some(6));

// Either way the attribute is kept, as preserved content of the Claim.
let document = tolerant.into_document();
assert!(document.get("c1").is_some_and(|c1| c1.has_preserved_content()));

// A strict write refuses a document that holds preserved content.
let refused = xmi::write(&document, &WriteOptions::default()).expect_err("it is refused");
assert_eq!(refused.code(), Code::XMI_STRICT_SAVE_REFUSED);

// A tolerant write writes the attribute back.
let text = xmi::write(&document, &WriteOptions::default().with_mode(Mode::Tolerant))?;
assert!(text.contains(r#"tool:colour="green""#));
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **Open another tool's file:** read in tolerant mode, and look at the Warnings.
- **Keep what a tolerant read kept:** write in tolerant mode.
- **Hand a file on as SACM 2.3:** write in strict mode.
- **Known gap.** A namespace prefix used only inside a preserved attribute's value is not
  declared on a tolerant write, so its binding is lost (LIB-COMPAT-013).
- **Tolerant still refuses some input,** such as malformed XML, a DOCTYPE, a root that is
  not a SACM package, a SACM 2.4 root, a repeated `xmi:id` and input over the read limits.

## Examples

Four programs in `crates/sacm/examples/` each show one task from start to end. Each
runs without arguments, from a checkout of the repository:

```sh
cargo run -p sacm --example read_and_validate
```

| Program | What it shows |
|---|---|
| `read_and_validate.rs` | Read an XMI file, validate it and print the diagnostics. |
| `build_and_save.rs` | Build an assurance case in code and write it as XMI. |
| `edit_in_place.rs` | Read a file, change it and save it again. |
| `translate_diagnostics.rs` | Show diagnostics in another language, with the catalog. |

The other three run the same way: `--example build_and_save`, `--example edit_in_place`
and `--example translate_diagnostics`.

## More

- **The concepts.** The [SACM primer](https://github.com/lasrod/libSACM/blob/main/crates/sacm/PRIMER.md).
- **The API in full.** The [crate documentation](https://docs.rs/sacm).
- **What is verified.** The
  [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md),
  requirement by requirement.
