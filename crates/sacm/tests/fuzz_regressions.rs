//! The stable replay of the fuzz targets (LIB-SEC-004; ADR 0046 D2).
//!
//! The fuzz targets in `fuzz/` run on a pinned nightly only. This file runs
//! their oracles, [`oracle_xmi::check_xmi`] and [`oracle_serde::check_serde`],
//! on stable and the MSRV, over:
//!
//! - every regression input in `tests/data/fuzz/xmi` and `tests/data/fuzz/serde`
//!   (a crasher the fuzzer found becomes one, at most 64 KiB);
//! - every `.xmi` file of the sacm and sacm-cli test data;
//! - arbitrary bytes, and fixtures and serde goldens mutated with the edits a
//!   fuzzer makes, from fixed seeds (`PROPTEST_CASES` and `PROPTEST_RNG_SEED`
//!   override them, as for `properties.rs`).

#[path = "common/oracle_xmi.rs"]
mod oracle_xmi;

#[cfg(feature = "serde")]
#[path = "common/oracle_serde.rs"]
mod oracle_serde;

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use proptest::collection::vec;
use proptest::prelude::{ProptestConfig, Strategy, any, prop_oneof};
use proptest::test_runner::{RngSeed, TestCaseError, TestRunner};

const SEC_004: &str = "LIB-SEC-004";

/// The sacm crate's test data.
const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

/// The sacm-cli crate's test data.
const CLI_DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sacm-cli/tests/data");

/// The 12 tokens of `fuzz/xmi.dict`, which [`Edit::InsertToken`] inserts.
const TOKENS: [&str; 12] = [
    "<",
    ">",
    "/>",
    "\"",
    "&",
    "xmi:id=\"",
    "xmi:type=\"sacm:Claim\"",
    "href=\"#",
    "xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    "<xmi:Extension>",
    "<xmi:Difference><target ",
    "<!DOCTYPE x>",
];

/// The configuration of a property: `cases` cases from the fixed `seed`,
/// unless `PROPTEST_CASES` or `PROPTEST_RNG_SEED` is set, and no failure
/// files (ADR 0046 D1).
fn config(cases: u32, seed: u64) -> ProptestConfig {
    let mut config = ProptestConfig::default();
    if std::env::var_os("PROPTEST_CASES").is_none() {
        config.cases = cases;
    }
    if std::env::var_os("PROPTEST_RNG_SEED").is_none() {
        config.rng_seed = RngSeed::Fixed(seed);
    }
    config.failure_persistence = None;
    config
}

/// Runs `test` over `strategy` under [`config`]`(cases, seed)`, failing with
/// the shrunk case.
fn run<S>(cases: u32, seed: u64, strategy: &S, test: impl Fn(S::Value) -> Result<(), TestCaseError>)
where
    S: Strategy,
    S::Value: Debug,
{
    let mut runner = TestRunner::new(config(cases, seed));
    if let Err(error) = runner.run(strategy, test) {
        panic!("{SEC_004}: {error}");
    }
}

/// The files under `dir`, recursively, that `keep` accepts, sorted by path.
fn files_under(dir: &Path, keep: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = std::fs::read_dir(&next).unwrap_or_else(|error| {
            panic!("{SEC_004}: setup: cannot list {}: {error}", next.display())
        });
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!("{SEC_004}: setup: cannot list {}: {error}", next.display())
                })
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if keep(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Every file under `dir`, failing when there is none.
fn every_file(dir: &Path) -> Vec<PathBuf> {
    let found = files_under(dir, |_| true);
    assert!(
        !found.is_empty(),
        "{SEC_004}: setup: no file under {}",
        dir.display()
    );
    found
}

fn bytes_of(path: &Path) -> Vec<u8> {
    std::fs::read(path)
        .unwrap_or_else(|error| panic!("{SEC_004}: setup: cannot read {}: {error}", path.display()))
}

/// Every `.xmi` file of the sacm and sacm-cli test data, in sorted path order,
/// with its bytes.
static FIXTURES: LazyLock<Vec<(PathBuf, Vec<u8>)>> = LazyLock::new(|| {
    let is_xmi = |path: &Path| path.extension().is_some_and(|extension| extension == "xmi");
    let mut paths = files_under(Path::new(DATA), is_xmi);
    paths.extend(files_under(Path::new(CLI_DATA), is_xmi));
    paths.sort();
    assert!(
        paths.len() > 50,
        "{SEC_004}: setup: only {} .xmi fixtures",
        paths.len()
    );
    paths
        .into_iter()
        .map(|path| {
            let bytes = bytes_of(&path);
            (path, bytes)
        })
        .collect()
});

// ---------------------------------------------------------------------------
// The edits a fuzzer makes
// ---------------------------------------------------------------------------

/// One edit of an input. Each position is taken modulo the input's length
/// (plus one where the end is a position too), so every edit applies.
#[derive(Clone, Debug)]
enum Edit {
    /// Flips the bit `bit % 8` of the byte at `pos`.
    Flip(u32, u8),
    /// Inserts `byte` at `pos`.
    Insert(u32, u8),
    /// Deletes up to `len` bytes from `pos`.
    Delete(u32, u8),
    /// Repeats up to `len` bytes from `pos` right after them.
    Duplicate(u32, u8),
    /// Keeps the first `pos` bytes.
    Truncate(u32),
    /// Inserts the token `TOKENS[k % 12]` at `pos`.
    InsertToken(u32, u8),
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (any::<u32>(), any::<u8>()).prop_map(|(pos, bit)| Edit::Flip(pos, bit)),
        (any::<u32>(), any::<u8>()).prop_map(|(pos, byte)| Edit::Insert(pos, byte)),
        (any::<u32>(), any::<u8>()).prop_map(|(pos, len)| Edit::Delete(pos, len)),
        (any::<u32>(), any::<u8>()).prop_map(|(pos, len)| Edit::Duplicate(pos, len)),
        any::<u32>().prop_map(Edit::Truncate),
        (any::<u32>(), any::<u8>()).prop_map(|(pos, k)| Edit::InsertToken(pos, k)),
    ]
}

/// `pos` modulo `bound`, or 0 when `bound` is 0.
fn at(pos: u32, bound: usize) -> usize {
    usize::try_from(pos).map_or(0, |pos| pos.checked_rem(bound).unwrap_or(0))
}

/// `input` with `edits` applied in order.
fn edited(input: &[u8], edits: &[Edit]) -> Vec<u8> {
    let mut bytes = input.to_vec();
    for edit in edits {
        let len = bytes.len();
        match *edit {
            Edit::Flip(pos, bit) => {
                if let Some(byte) = bytes.get_mut(at(pos, len)) {
                    *byte ^= 1 << (bit % 8);
                }
            }
            Edit::Insert(pos, byte) => bytes.insert(at(pos, len + 1), byte),
            Edit::Delete(pos, count) => {
                let from = at(pos, len + 1);
                let to = from.saturating_add(usize::from(count)).min(len);
                bytes.drain(from..to);
            }
            Edit::Duplicate(pos, count) => {
                let from = at(pos, len + 1);
                let to = from.saturating_add(usize::from(count)).min(len);
                let copy = bytes[from..to].to_vec();
                bytes.splice(to..to, copy);
            }
            Edit::Truncate(pos) => bytes.truncate(at(pos, len + 1)),
            Edit::InsertToken(pos, k) => {
                let token = TOKENS[usize::from(k) % TOKENS.len()].as_bytes();
                let to = at(pos, len + 1);
                bytes.splice(to..to, token.iter().copied());
            }
        }
    }
    bytes
}

// ---------------------------------------------------------------------------
// The replays
// ---------------------------------------------------------------------------

#[test]
fn lib_sec_004_every_regression_input_passes_the_oracles() {
    for path in every_file(&Path::new(DATA).join("fuzz/xmi")) {
        oracle_xmi::check_xmi(&bytes_of(&path));
    }
    let serde_inputs = every_file(&Path::new(DATA).join("fuzz/serde"));
    #[cfg(feature = "serde")]
    for path in serde_inputs {
        oracle_serde::check_serde(&bytes_of(&path));
    }
    #[cfg(not(feature = "serde"))]
    let _ = serde_inputs;
}

#[test]
fn lib_sec_004_every_fixture_passes_the_xmi_oracle() {
    for (_, bytes) in FIXTURES.iter() {
        oracle_xmi::check_xmi(bytes);
    }
}

#[test]
fn lib_sec_004_arbitrary_bytes_pass_the_xmi_oracle() {
    run(512, 40_411, &vec(any::<u8>(), 0..2048), |bytes| {
        oracle_xmi::check_xmi(&bytes);
        Ok(())
    });
}

#[test]
fn lib_sec_004_mutated_fixtures_pass_the_xmi_oracle() {
    let fixtures = &*FIXTURES;
    run(
        256,
        40_412,
        &(0..fixtures.len(), vec(edit(), 0..=8)),
        |(index, edits)| {
            oracle_xmi::check_xmi(&edited(&fixtures[index].1, &edits));
            Ok(())
        },
    );
}

#[cfg(feature = "serde")]
#[test]
fn lib_sec_004_mutated_serde_goldens_pass_the_serde_oracle() {
    let goldens: Vec<Vec<u8>> = every_file(&Path::new(DATA).join("golden/serde"))
        .iter()
        .map(|path| bytes_of(path))
        .collect();
    run(
        256,
        40_413,
        &(0..goldens.len(), vec(edit(), 0..=8)),
        |(index, edits)| {
            oracle_serde::check_serde(&edited(&goldens[index], &edits));
            Ok(())
        },
    );
}
