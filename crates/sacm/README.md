# sacm

Read, edit, validate and write assurance cases in the
[OMG Structured Assurance Case Metamodel (SACM) 2.3](https://www.omg.org/spec/SACM/2.3).

> **Status: early development.** The API is not stable yet. What is
> implemented and how it is tested is tracked requirement by requirement in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

## What works today

- `sacm::xmi::read` reads `AssuranceCasePackage`, `ArgumentPackage`, `ArtifactPackage` and
  `TerminologyPackage` roots and the packages and Claims in them, with their names, `gid`s and each Assertion's
  `assertionDeclaration`, and the descriptions, notes, implementation constraints and
  tagged values of each (`SacmElement::descriptions`, `notes`, `tagged_values` and the
  like), with their `content` and, on a tagged value, `key` (`SacmElement::content`,
  `SacmElement::key`). Every element's `isCitation`, `citedElement`, `isAbstract` and
  `abstractForm` are read too (`SacmElement::is_citation`, `cited_element` and the like),
  in the attribute or the element form; a reference is held as the id it names, and one
  that names no element is kept. In an `ArgumentPackage` it also reads
  `ArgumentReasoning`, `ArtifactReference` and `ArgumentGroup` elements and the five
  asserted relationships (`AssertedInference`, `AssertedEvidence`, `AssertedContext`,
  `AssertedArtifactSupport`, `AssertedArtifactContext`), with each relationship's
  sources, target, reasoning and `isCounter` (`SacmElement::sources`, `target`,
  `reasoning`, `is_counter`), each Assertion's meta-claims (`SacmElement::meta_claims`),
  each reasoning's `structure`, each artifact reference's referenced elements
  (`SacmElement::referenced_artifact_elements`) and each group's members
  (`SacmElement::group_members`). In an `ArtifactPackage` it reads `Technique`,
  `Participant`, `Property`, `ArtifactGroup`, `Artifact`, `Event`, `Resource`, `Activity`
  and `ArtifactAssetRelationship` elements, with each asset's properties
  (`SacmElement::properties`), each group's members, each relationship's sources and
  targets (`SacmElement::sources`, `targets`), an Artifact's `version`, the `date` of an Artifact or an Event, an
  Activity's `startTime` and `endTime`, each held exactly as written, and a Resource's
  `location` (`SacmElement::version`, `date`, `start_time`, `end_time`, `location`). In a
  `TerminologyPackage` it reads `Category`, `Term`, `Expression` and `TerminologyGroup`
  elements, with each Term's or Expression's `value` and categories, a Term's `origin` and
  `externalReference`, and an Expression's elements (`SacmElement::value`, `categories`,
  `origin`, `external_reference`, `expression_elements`). The interfaces and bindings of
  every package are read as roots and as children, with each interface's `implements`,
  each binding's participants and each package's `interface` (`SacmElement::implements`,
  `participant_packages`, `interfaces`). A `name`, or a value of a
  `content`, `key` or `location`, may be an `ExpressionLangString` (§8.4) that links an
  ExpressionElement (`LangString::expression`, `SacmElement::expressions`). Content that
  is not SACM 2.3 where it stands, such as
  a vendor attribute or element, is kept as written as preserved content, with
  SACM-XMI-003 (`SacmElement::has_preserved_content`). A tolerant read, the default,
  opens every well-formed file with SACM 2.3 package roots: what a strict read refuses,
  such as a value that is no literal or conforming content not read yet, it keeps as
  preserved content with a Warning, never dropped (`Document::has_preserved_content`),
  and it reads the four names formal/23-05-08 and ptc/22-03-13 spell differently,
  `ArtifactAssertedRelationship`, an Event's `occurece`, `argumentationElement` and
  `TerminologyElement`, as the
  names `write` writes, with a SACM-XMI-013 Warning; a strict read keeps them as preserved
  content with an Error.
  An element without an `xmi:id` gets a generated one, such as `claim_1`, which `write`
  saves; see the Ids section of the `sacm::xmi::read` docs.
- `sacm::xmi::write` writes a document deterministically, preserved content included.
  A strict save, the default, refuses output that would not be schema-valid and a
  document that holds preserved content; it never validates.
- `sacm::validation::validate` reports each element that has no name, each `content`,
  `key` or `location` text that is empty or gives one language twice, each tagged value without a key,
  each id that is not an XML `NCName`, each `gid` two elements share, each element with
  more than one description, each `ArgumentPackage` that nests one and also holds
  other argumentation elements, each reference that names no element or an element of the
  wrong kind, each element with a `citedElement` whose `isCitation` is not true, each
  `abstractForm` that breaks a §8.2 rule, each asserted relationship without a source or
  a target, each evidence or artifact relationship whose ends are not artifact references
  (§11.15, §11.17, §11.18), each Term or Expression without a value, each
  ExpressionLangString without an `expression` or with both an `expression` and a
  non-empty content, each concrete Expression whose elements are abstract, and each id a
  reference end repeats, as diagnostics with stable codes.
- `sacm::edit::apply` creates AssuranceCasePackages, ArgumentPackages, Claims,
  ArgumentReasonings, ArtifactReferences, ArgumentGroups and the five asserted
  relationships (`sacm::edit::CreateAssertedInference` and the like), ArtifactPackages,
  Properties, Techniques, Participants, ArtifactGroups, Artifacts, Events, Resources,
  Activities and ArtifactAssetRelationships (`sacm::edit::CreateArtifactPackage`,
  `sacm::edit::CreateArtifactAssetRelationship` and the like), TerminologyPackages,
  Categories, Terms, Expressions and TerminologyGroups (`sacm::edit::CreateTerm` and the
  like), the interfaces and bindings of every package
  (`sacm::edit::CreateTerminologyPackageInterface` and the like), and descriptions,
  implementation constraints, notes and tagged values on them
  (`sacm::edit::CreateNote`, `sacm::edit::CreateTaggedValue` and the like); a refused
  edit changes nothing, and an edit refuses a reference that names no element or an
  element its end's declared type does not admit. The stricter §11.15, §11.17 and §11.18
  rules, that evidence and artifact relationships link ArtifactReferences, are left to
  `sacm::validation::validate`. It also changes existing elements of every kind: their
  names, `gid`s, flags and `assertionDeclaration`s (`sacm::edit::SetName` and the
  like), the `content` and `key` of descriptions, implementation constraints, notes
  and tagged values (`sacm::edit::SetContent`, `sacm::edit::SetKey`), an Artifact's
  `version`, the `date` of an Artifact or an Event, a Resource's `location` and an
  Activity's `startTime` and `endTime` (`sacm::edit::SetVersion`, `SetDate`,
  `SetLocation`, `SetStartTime`, `SetEndTime`), a Term's or an Expression's `value` and a
  Term's `externalReference` (`SetValue`, `SetExternalReference`), every reference
  end, within its bounds (`sacm::edit::SetReferences`, `AddReference`, `RemoveReference`), and the
  order of an element's children or of the roots (`sacm::edit::ReorderElements`).
  `sacm::edit::MoveElement` moves an element, with everything it contains, under another
  parent or to the roots, and `sacm::edit::CopyElements` copies elements of another
  document, such as a draft, with fresh ids where theirs clash.
- `Document::referrers` lists the typed references that name an id, and
  `SacmElement::flag` and `SacmElement::references`, with `Flag::ALL` and
  `ReferenceEnd::ALL`, read an element's flags and ends without naming each one.
- `sacm::compare::semantic_compare` lists every difference in SACM content between two
  documents.

The [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md)
tracks every requirement: what is built, what is planned, and the tests behind each.

Planned:

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

A strict save refuses output that would not be schema-valid, an `xmi:id` that is no
`NCName` or a reference to no `xmi:id` it writes, as `WriteError::Invalid`, and a document
that holds preserved content, as `WriteError::PreservedContent`. It never validates: a
draft with an open model Error, such as an element with no name, saves strictly. Call
`validation::validate` for model checks.

Which `xmi::Mode` to use: read in tolerant mode, the default, to open files from other
tools: it keeps what a strict read refuses, with a Warning. A strict read refuses what it
does not read, and reports the variations it recognises as Errors instead of Warnings.
Neither mode validates what it reads. To check that a file conforms, read it in strict
mode, run `validation::validate`, and test both lists with `diagnostic::has_errors`: a
tolerant read reports what a strict read refuses as Warnings and keeps it as preserved
content (`Document::has_preserved_content`), so its diagnostics alone are no conformance
check. Save in strict mode, the default, for a file you hand on; save in tolerant mode to
keep what a tolerant read preserved.

See the [crate documentation](https://docs.rs/sacm) for the full API.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
