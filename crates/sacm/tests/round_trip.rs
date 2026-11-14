//! Round-trip guarantees, checked with `sacm::compare::semantic_compare`
//! (D-RT; ADR 0020):
//! - LIB-RT-002: every valid fixture under `tests/data/sacm23/xmi/` reads,
//!   writes, reads back equal and writes the same bytes again, and a golden
//!   file writes back to its own bytes;
//! - LIB-RT-003: a document built with `edit::apply` only writes, reads back,
//!   validates with no Error and equals the built document, also after its
//!   Claims are given each of the six declarations.
//!
//! The fixtures are found by walking the directory, so a new valid fixture is
//! covered without a change here. The walk skips the `invalid/` directory and
//! the files on [`PENDING`], and nothing else: every other file under the
//! directory is a fixture in scope. A pending file holds kinds no verified row
//! reads yet; it must still be refused as `ReadError::Unsupported`, and the
//! test fails once it reads, so that it leaves the list and joins the round
//! trip.
//!
//! LIB-RT-002 writes in tolerant mode: several fixtures and golden files are
//! nameless, which a strict write refuses (LIB-COMPAT-007), and the round trip
//! is about content, not the save mode. LIB-RT-003 writes with
//! `WriteOptions::default()`, strict, because a built document validates.
//! Documents for LIB-RT-003 use the public edit API only; the
//! `test-support` seam `set_assertion_declaration` is used for the
//! declarations, which the edit API cannot set yet.

use std::path::{Path, PathBuf};

use sacm::compare::semantic_compare;
use sacm::diagnostic;
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, Operation};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, LangString, SacmElement,
};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteOptions};

/// The directory of the XMI fixtures.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi");
/// The one directory under [`FIXTURES`] that is out of scope.
const INVALID: &str = "invalid";
/// The directory of the golden files, under [`FIXTURES`].
const GOLDEN: &str = "golden";
/// The fixtures out of the round trip until the reader reads their kinds,
/// relative to [`FIXTURES`], each with why it is pending (LIB-RT-002).
const PENDING: [(&str, &str); 1] = [(
    // A SACM23-XMI-007 writer golden; `xmi::read` refuses it as Unsupported.
    "golden/nested.golden.sacm.xmi",
    "holds an ArgumentPackageInterface, an ArtifactPackage, an Artifact and a Property, \
     which no verified row reads yet",
)];

/// The three states of one `LangString` field: absent, empty and non-empty
/// (non-ASCII).
const FIELD_STATES: [Option<&str>; 3] = [None, Some(""), Some("Säkerhet ✓ 安全")];

// ---------------------------------------------------------------------------
// LIB-RT-002 helpers
// ---------------------------------------------------------------------------

/// Every file under `dir`, recursively, except under a directory named
/// [`INVALID`], sorted by path. Fails the test if a directory cannot be read.
fn fixtures_under(row: &str, dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = std::fs::read_dir(&next)
            .unwrap_or_else(|error| panic!("{row}: cannot list {}: {error}", next.display()));
        for entry in entries {
            let entry = entry
                .unwrap_or_else(|error| panic!("{row}: cannot list {}: {error}", next.display()));
            let path = entry.path();
            if path.is_dir() {
                if entry.file_name() != INVALID {
                    pending.push(path);
                }
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Whether `path` is on [`PENDING`].
fn is_pending(path: &Path) -> bool {
    let relative = shown(path);
    PENDING.iter().any(|&(pending, _)| pending == relative)
}

/// The fixtures in scope under `dir`: the walk without the [`PENDING`]
/// files, failing the test if there are none.
fn fixtures_in(row: &str, dir: &Path) -> Vec<PathBuf> {
    let mut found = fixtures_under(row, dir);
    found.retain(|path| !is_pending(path));
    assert!(
        !found.is_empty(),
        "{row}: no fixture was found under {}",
        dir.display()
    );
    found
}

/// The fixtures in scope, failing the test if there are none.
fn fixtures(row: &str) -> Vec<PathBuf> {
    fixtures_in(row, Path::new(FIXTURES))
}

/// `path` relative to [`FIXTURES`], with `/` separators, for messages.
fn shown(path: &Path) -> String {
    let relative = path.strip_prefix(FIXTURES).unwrap_or(path);
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Reads `bytes` as XMI, or says why not.
fn read(bytes: &[u8]) -> Result<Document, String> {
    xmi::read(bytes, &ReadOptions::default())
        .map(xmi::ReadOutput::into_document)
        .map_err(|error| format!("does not read: {error} ({error:?})"))
}

/// Writes `doc` in tolerant mode, or says why not. Tolerant, because some
/// fixtures are nameless and a strict write refuses a validation Error
/// (LIB-COMPAT-007); both modes write the same bytes for a document both write.
fn write(doc: &Document) -> Result<String, String> {
    xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant))
        .map_err(|error| format!("does not write: {error} ({error:?})"))
}

/// Read → write → read → write for one fixture's bytes: `Ok` with the first
/// written text when the reloaded document is equal and the second write
/// gives the same bytes, or the reason it is not.
fn round_trip(bytes: &[u8]) -> Result<String, String> {
    let first = read(bytes)?;
    let written = write(&first).map_err(|error| format!("read once, then {error}"))?;
    let second = read(written.as_bytes())
        .map_err(|error| format!("written once, the output {error}\n{written}"))?;
    let differences = semantic_compare(&first, &second);
    if !differences.is_empty() {
        return Err(format!(
            "reads back with differences {differences:?}\n{written}"
        ));
    }
    let again = write(&second).map_err(|error| format!("read back, then {error}"))?;
    if again != written {
        return Err(format!(
            "the second write differs from the first\nfirst:\n{written}\nsecond:\n{again}"
        ));
    }
    Ok(written)
}

/// Reads the fixture at `path`.
fn bytes_of(row: &str, path: &Path) -> Vec<u8> {
    std::fs::read(path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", shown(path)))
}

// ---------------------------------------------------------------------------
// LIB-RT-002
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_002_the_fixture_walk_finds_every_valid_fixture_and_skips_only_invalid_and_pending() {
    let row = "LIB-RT-002";
    let found: Vec<String> = fixtures(row).iter().map(|path| shown(path)).collect();
    for named in [
        "argument-package-root-valid.sacm.xmi",
        "assurance-case-package-root-valid.sacm.xmi",
        "name-third-party-form-valid.sacm.xmi",
        "wrapped-roots-valid.sacm.xmi",
        "golden/named.golden.sacm.xmi",
        "golden/needs-support-claim.golden.sacm.xmi",
        "golden/several-roots.golden.sacm.xmi",
        "golden/single-root.golden.sacm.xmi",
    ] {
        assert!(
            found.iter().any(|path| path == named),
            "{row}: the walk does not find {named}; it finds {found:?}"
        );
    }
    for path in &found {
        assert!(
            !path.split('/').any(|part| part == INVALID),
            "{row}: the walk includes {path}, under {INVALID}/"
        );
        assert!(
            !PENDING.iter().any(|&(pending, _)| pending == path),
            "{row}: the walk includes the pending file {path}"
        );
    }

    // Everything else is found: every file in the tree minus `invalid/` and
    // the pending list, each of which is in the tree.
    let mut everything = Vec::new();
    let mut pending = vec![PathBuf::from(FIXTURES)];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                everything.push(shown(&path));
            }
        }
    }
    for (pending, _) in PENDING {
        assert!(
            everything.iter().any(|path| path == pending),
            "{row}: the pending file {pending} is not in the fixture tree"
        );
    }
    let invalid_prefix = format!("{INVALID}/");
    let mut expected: Vec<String> = everything
        .into_iter()
        .filter(|path| !path.starts_with(&invalid_prefix))
        .filter(|path| !PENDING.iter().any(|&(pending, _)| pending == path))
        .collect();
    expected.sort();
    let mut found_sorted = found;
    found_sorted.sort();
    assert_eq!(
        found_sorted, expected,
        "{row}: the walk skips something other than {INVALID}/ and the pending list"
    );
}

#[test]
fn lib_rt_002_each_pending_fixture_is_still_refused_as_unsupported() {
    let row = "LIB-RT-002";
    for (pending, why) in PENDING {
        let path = Path::new(FIXTURES).join(pending);
        match xmi::read(&bytes_of(row, &path), &ReadOptions::default()) {
            Err(ReadError::Unsupported { .. }) => {}
            Ok(_) => panic!(
                "{row}: the pending file {pending} now reads; take it off the pending list \
                 (pending because it {why}) so that it joins the round trip"
            ),
            Err(error) => panic!(
                "{row}: the pending file {pending} is refused, but not as Unsupported: \
                 {error} ({error:?})"
            ),
        }
    }
}

#[test]
fn lib_rt_002_the_fixture_walk_fails_when_it_finds_no_fixture() {
    let row = "LIB-RT-002";
    // A scratch tree whose only file is under `invalid/`.
    let tree = std::env::temp_dir().join(format!("sacm-lib-rt-002-{}", std::process::id()));
    let invalid = tree.join(INVALID);
    std::fs::create_dir_all(&invalid).unwrap_or_else(|error| {
        panic!("{row}: setup: cannot create {}: {error}", invalid.display())
    });
    std::fs::write(invalid.join("x-invalid.sacm.xmi"), "<x/>").unwrap_or_else(|error| {
        panic!(
            "{row}: setup: cannot write under {}: {error}",
            invalid.display()
        )
    });

    let found = fixtures_under(row, &tree);
    let refused = std::panic::catch_unwind(|| fixtures_in(row, &tree));
    let _ = std::fs::remove_dir_all(&tree);

    assert_eq!(
        found,
        Vec::<PathBuf>::new(),
        "{row}: the walk yields a file under {INVALID}/"
    );
    assert!(
        refused.is_err(),
        "{row}: the fixture list of a tree with no valid fixture does not fail"
    );
}

#[test]
fn lib_rt_002_every_valid_fixture_reads_writes_reads_back_equal_and_writes_the_same_bytes() {
    let row = "LIB-RT-002";
    let mut failures = Vec::new();
    let all = fixtures(row);
    for path in &all {
        if let Err(reason) = round_trip(&bytes_of(row, path)) {
            failures.push(format!("{}: {reason}", shown(path)));
        }
    }
    assert!(
        failures.is_empty(),
        "{row}: {} of {} valid fixtures do not round-trip:\n{}",
        failures.len(),
        all.len(),
        failures.join("\n\n")
    );
}

#[test]
fn lib_rt_002_every_golden_file_writes_back_to_its_own_bytes() {
    let row = "LIB-RT-002";
    let golden_dir = Path::new(FIXTURES).join(GOLDEN);
    let golden: Vec<PathBuf> = fixtures(row)
        .into_iter()
        .filter(|path| path.starts_with(&golden_dir))
        .collect();
    assert!(
        !golden.is_empty(),
        "{row}: no golden file was found under {}",
        golden_dir.display()
    );
    let mut failures = Vec::new();
    for path in &golden {
        let bytes = bytes_of(row, path);
        match round_trip(&bytes) {
            Ok(written) if written.as_bytes() == bytes.as_slice() => {}
            Ok(written) => failures.push(format!(
                "{}: writes back to other bytes:\n{written}",
                shown(path)
            )),
            Err(reason) => failures.push(format!("{}: {reason}", shown(path))),
        }
    }
    assert!(
        failures.is_empty(),
        "{row}: {} of {} golden files do not write back to their own bytes:\n{}",
        failures.len(),
        golden.len(),
        failures.join("\n\n")
    );
}

// ---------------------------------------------------------------------------
// LIB-RT-003 helpers
// ---------------------------------------------------------------------------

/// The row the LIB-RT-003 tests verify.
const RT_003: &str = "LIB-RT-003";

/// A name with exactly the given `lang` and `content`, each absent when `None`.
fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut value = LangString::default();
    value.set_lang(lang.map(str::to_owned));
    value.set_content(content.map(str::to_owned));
    value
}

/// A non-empty English name.
fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Applies a create operation and returns the id of the element it created,
/// or fails the test as a setup error.
fn create(doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => match mutation.created_ids().first() {
            Some(created) => created.clone(),
            None => panic!("{RT_003}: setup: {operation:?} created no id"),
        },
        Err(error) => panic!("{RT_003}: setup could not apply {operation:?}: {error}"),
    }
}

/// One `AssuranceCasePackage` root with a generated id and nothing in it.
fn single_root() -> Document {
    let mut doc = Document::new();
    create(&mut doc, CreateAssuranceCasePackage::root(en("Case")));
    doc
}

/// Three roots, depth 4, every create operation (`root` and `new` of both
/// packages, and `CreateClaim`), generated and caller ids, and names in all
/// nine absent, empty and non-ASCII `(lang, content)` states.
fn deep_and_wide() -> Document {
    let mut doc = Document::new();
    let case = create(
        &mut doc,
        CreateAssuranceCasePackage::root(en("Safety case")),
    );
    let sub_case = create(
        &mut doc,
        CreateAssuranceCasePackage::new(&case, LangString::default()).with_id("sub_case"),
    );
    let arguments = create(
        &mut doc,
        CreateArgumentPackage::new(&sub_case, lang_string(Some(""), Some(""))),
    );
    let inner = create(
        &mut doc,
        CreateArgumentPackage::new(&arguments, en("Inner")).with_id("påstående_paket"),
    );
    let mut given = 0;
    for (n, lang) in FIELD_STATES.into_iter().enumerate() {
        for content in FIELD_STATES {
            let name = lang_string(lang, content);
            let operation = if n == 1 {
                given += 1;
                CreateClaim::new(&inner, name).with_id(format!("claim_given_{given}"))
            } else {
                CreateClaim::new(&inner, name)
            };
            create(&mut doc, operation);
        }
    }
    // A second nested ArgumentPackage after the first, holding a Claim. Not a
    // Claim directly in the outer package: next to a nested ArgumentPackage
    // that is a SACM-PKG-001 Error (SACM23-VAL-012, clause 11.4), and these
    // documents must validate with no Error.
    let after = create(
        &mut doc,
        CreateArgumentPackage::new(&arguments, en("After")).with_id("after_nested"),
    );
    create(&mut doc, CreateClaim::new(&after, LangString::new("")));
    let second = create(&mut doc, CreateArgumentPackage::root(en("Second root")));
    create(&mut doc, CreateClaim::new(&second, en("C")));
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(LangString::new("Ünïcödé")).with_id("third_root"),
    );
    doc
}

/// Six roots of alternating kinds, each holding the next kind down, with
/// generated ids only.
fn many_roots() -> Document {
    let mut doc = Document::new();
    for n in 0..3 {
        let case = create(
            &mut doc,
            CreateAssuranceCasePackage::root(en(&format!("Case {n}"))),
        );
        let arguments = create(&mut doc, CreateArgumentPackage::new(&case, en("Arguments")));
        create(&mut doc, CreateClaim::new(&arguments, en("Claim")));
        let root = create(
            &mut doc,
            CreateArgumentPackage::root(lang_string(Some("sv"), None)),
        );
        create(
            &mut doc,
            CreateClaim::new(&root, lang_string(None, Some("Påstående"))),
        );
    }
    doc
}

/// The deterministic LIB-RT-003 set, by description.
fn built_set() -> Vec<(&'static str, Document)> {
    vec![
        ("one generated root", single_root()),
        (
            "three roots, depth 4, every operation and name state",
            deep_and_wide(),
        ),
        ("six roots of alternating kinds", many_roots()),
    ]
}

/// The ids of every Claim in `doc`, in document order.
fn claims(doc: &Document) -> Vec<ElementId> {
    doc.elements()
        .filter(|element| element.kind() == ElementKind::Claim)
        .map(|element| element.id().clone())
        .collect()
}

/// Sets `declaration` on every Claim of `doc` through the seam.
fn declare_all(doc: &mut Document, mut declaration: impl FnMut(usize) -> AssertionDeclaration) {
    for (n, claim) in claims(doc).iter().enumerate() {
        let value = declaration(n);
        if let Err(error) = doc.set_assertion_declaration(claim, value) {
            panic!("{RT_003}: setup could not set {value} on {claim:?}: {error}");
        }
    }
}

/// Asserts the LIB-RT-003 guarantee for `built`: it writes, reads back,
/// validates with no Error and the reloaded document equals it.
fn assert_round_trips(what: &str, built: &Document) {
    assert!(
        !diagnostic::has_errors(&validation::validate(built)),
        "{RT_003}: {what}: setup: the built document has a validation error: {:?}",
        validation::validate(built)
    );
    let written = xmi::write(built, &WriteOptions::default())
        .unwrap_or_else(|error| panic!("{RT_003}: {what}: does not write: {error} ({error:?})"));
    let output = xmi::read(written.as_bytes(), &ReadOptions::default()).unwrap_or_else(|error| {
        panic!("{RT_003}: {what}: the output does not read back: {error}\n{written}")
    });
    let mut findings = output.diagnostics().to_vec();
    let reloaded = output.into_document();
    findings.extend(validation::validate(&reloaded));
    assert!(
        !diagnostic::has_errors(&findings),
        "{RT_003}: {what}: the reloaded document has an Error: {findings:?}\n{written}"
    );
    assert_eq!(
        semantic_compare(built, &reloaded),
        [],
        "{RT_003}: {what}: the reloaded document differs from the built one\n{written}"
    );
    assert!(
        *built == reloaded,
        "{RT_003}: {what}: built == reloaded is false\n{written}"
    );
}

// ---------------------------------------------------------------------------
// LIB-RT-003
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_003_documents_built_with_edit_round_trip_equal_and_valid() {
    let set = built_set();
    let deep = &set[1].1;
    assert!(
        deep.roots().count() >= 2
            && deep.elements().any(|element| {
                element
                    .parent()
                    .and_then(SacmElement::parent)
                    .and_then(SacmElement::parent)
                    .is_some()
            }),
        "{RT_003}: setup: the set has no document with two roots and depth 3"
    );
    for (what, built) in &set {
        assert_round_trips(what, built);
    }
}

#[test]
fn lib_rt_003_generated_ids_are_kept_as_written() {
    let built = deep_and_wide();
    let written = xmi::write(&built, &WriteOptions::default())
        .unwrap_or_else(|error| panic!("{RT_003}: does not write: {error}"));
    let reloaded = xmi::read(written.as_bytes(), &ReadOptions::default())
        .unwrap_or_else(|error| panic!("{RT_003}: does not read back: {error}\n{written}"))
        .into_document();
    for generated in [
        "assurance_case_package_1",
        "argument_package_1",
        "argument_package_2",
        "claim_1",
        "claim_8",
    ] {
        assert!(
            built.contains(generated),
            "{RT_003}: setup: the built document has no {generated:?}"
        );
        assert!(
            reloaded.contains(generated),
            "{RT_003}: the generated id {generated:?} is not read back\n{written}"
        );
    }
    for given in ["sub_case", "påstående_paket", "claim_given_1", "third_root"] {
        assert!(
            reloaded.contains(given),
            "{RT_003}: the caller id {given:?} is not read back\n{written}"
        );
    }
}

#[test]
fn lib_rt_003_documents_with_each_declaration_round_trip_equal_and_valid() {
    for &declaration in AssertionDeclaration::ALL {
        for (what, mut built) in built_set() {
            declare_all(&mut built, |_| declaration);
            assert_round_trips(&format!("{what}, every Claim {declaration}"), &built);
        }
    }
}

#[test]
fn lib_rt_003_documents_with_mixed_declarations_round_trip_equal_and_valid() {
    for (what, mut built) in built_set() {
        declare_all(&mut built, |n| {
            AssertionDeclaration::ALL[n % AssertionDeclaration::ALL.len()]
        });
        assert_round_trips(&format!("{what}, the six declarations in turn"), &built);
    }
}
