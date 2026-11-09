//! Tests for structured diagnostics: `Severity`, `Code`, `SourceLocation` and
//! `Diagnostic` in `sacm::diagnostic` (LIB-DIAG-001, ADR 0011).
//!
//! Diagnostics are built through the `test-support` constructors. The crate
//! dev-depends on itself with that feature, so these tests always see it.
//!
//! Not checked here:
//! - Item 4's "`Code` has no associated constants in this slice": a test cannot
//!   prove that an item is absent. The verifier checks the rustdoc item list.
//! - Item 9 (without `test-support`, users cannot build a `Diagnostic`, `Code`
//!   or `SourceLocation`; the module example compiles without the feature).
//!   These tests are always compiled with the feature on. The verifier checks
//!   the rustdoc item list and the module example built without the feature.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::model::ElementId;

/// A 1-based location that the test expects `SourceLocation::new` to accept.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("LIB-DIAG-001: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

// --- 1. Accessors ---------------------------------------------------------

#[test]
fn lib_diag_001_accessors_return_what_a_full_diagnostic_was_built_with() {
    let diagnostic = Diagnostic::new(
        Code::new("SACM-REF-001"),
        Severity::Error,
        "SACM23-REF-001",
        "the target \"claim_9\" does not resolve",
    )
    .with_clause("11.13")
    .with_affected(ids(&["reasoning_1", "claim_9"]))
    .with_location(loc(12, 34));

    assert_eq!(
        diagnostic.code(),
        Code::new("SACM-REF-001"),
        "LIB-DIAG-001: code() is not the code it was built with"
    );
    assert_eq!(
        diagnostic.code().as_str(),
        "SACM-REF-001",
        "LIB-DIAG-001: code().as_str() is not the code text"
    );
    assert_eq!(
        diagnostic.severity(),
        Severity::Error,
        "LIB-DIAG-001: severity() is not the severity it was built with"
    );
    assert_eq!(
        diagnostic.requirement_id(),
        "SACM23-REF-001",
        "LIB-DIAG-001: requirement_id() is not the row it was built with"
    );
    assert_eq!(
        diagnostic.clause(),
        Some("11.13"),
        "LIB-DIAG-001: clause() is not the clause it was built with"
    );
    assert_eq!(
        diagnostic.affected(),
        ids(&["reasoning_1", "claim_9"]).as_slice(),
        "LIB-DIAG-001: affected() is not the ids it was built with"
    );
    assert_eq!(
        diagnostic.location(),
        Some(loc(12, 34)),
        "LIB-DIAG-001: location() is not the location it was built with"
    );
    assert_eq!(
        diagnostic.message(),
        "the target \"claim_9\" does not resolve",
        "LIB-DIAG-001: message() is not the message it was built with"
    );
}

#[test]
fn lib_diag_001_accessors_of_a_minimal_diagnostic_have_no_clause_ids_or_location() {
    let diagnostic = Diagnostic::new(
        Code::new("SACM-XMI-008"),
        Severity::Warning,
        "SACM23-XMI-008",
        String::from("the document declares SACM 2.2"),
    );

    assert_eq!(
        diagnostic.code(),
        Code::new("SACM-XMI-008"),
        "LIB-DIAG-001: code() of a minimal diagnostic is wrong"
    );
    assert_eq!(
        diagnostic.severity(),
        Severity::Warning,
        "LIB-DIAG-001: severity() of a minimal diagnostic is wrong"
    );
    assert_eq!(
        diagnostic.requirement_id(),
        "SACM23-XMI-008",
        "LIB-DIAG-001: requirement_id() of a minimal diagnostic is wrong"
    );
    assert_eq!(
        diagnostic.clause(),
        None,
        "LIB-DIAG-001: a diagnostic built without a clause has one"
    );
    assert!(
        diagnostic.affected().is_empty(),
        "LIB-DIAG-001: a diagnostic built without ids has affected ids: {:?}",
        diagnostic.affected()
    );
    assert_eq!(
        diagnostic.location(),
        None,
        "LIB-DIAG-001: a diagnostic built without a location has one"
    );
    assert_eq!(
        diagnostic.message(),
        "the document declares SACM 2.2",
        "LIB-DIAG-001: message() of a minimal diagnostic is wrong"
    );
}

#[test]
fn lib_diag_001_each_builder_sets_only_its_own_field() {
    let base = Diagnostic::new(Code::new("C"), Severity::Info, "ROW-1", "m");

    let with_clause = base.clone().with_clause("8.2");
    assert_eq!(
        (
            with_clause.clause(),
            with_clause.affected().len(),
            with_clause.location()
        ),
        (Some("8.2"), 0, None),
        "LIB-DIAG-001: with_clause set more than the clause"
    );

    let with_affected = base.clone().with_affected(ids(&["x"]));
    assert_eq!(
        (
            with_affected.clause(),
            with_affected.affected(),
            with_affected.location()
        ),
        (None, ids(&["x"]).as_slice(), None),
        "LIB-DIAG-001: with_affected set more than the affected ids"
    );

    let with_location = base.clone().with_location(loc(2, 3));
    assert_eq!(
        (
            with_location.clause(),
            with_location.affected().len(),
            with_location.location()
        ),
        (None, 0, Some(loc(2, 3))),
        "LIB-DIAG-001: with_location set more than the location"
    );

    for (builder, built) in [
        ("with_clause", &with_clause),
        ("with_affected", &with_affected),
        ("with_location", &with_location),
    ] {
        assert_eq!(
            (
                built.code(),
                built.severity(),
                built.requirement_id(),
                built.message()
            ),
            (Code::new("C"), Severity::Info, "ROW-1", "m"),
            "LIB-DIAG-001: {builder} changed the code, severity, row or message"
        );
    }
}

// --- 2. Affected ids keep order and duplicates ----------------------------

#[test]
fn lib_diag_001_affected_keeps_the_given_order_and_duplicates() {
    // Neither sorted nor deduplicated: the emitter's order is the contract.
    // Adjacent duplicates catch `dedup`; separated ones catch a set.
    let given = ids(&["zeta", "zeta", "alpha", "zeta", "mu", "mu", "alpha"]);
    let diagnostic =
        Diagnostic::new(Code::new("C"), Severity::Error, "ROW-1", "m").with_affected(given.clone());
    assert_eq!(
        diagnostic.affected(),
        ids(&["zeta", "zeta", "alpha", "zeta", "mu", "mu", "alpha"]).as_slice(),
        "LIB-DIAG-001: affected() reordered or deduplicated the ids"
    );
}

#[test]
fn lib_diag_001_affected_accepts_any_iterator_of_ids() {
    let from_array = Diagnostic::new(Code::new("C"), Severity::Error, "ROW-1", "m")
        .with_affected([ElementId::from("b"), ElementId::from("a")]);
    let from_iterator = Diagnostic::new(Code::new("C"), Severity::Error, "ROW-1", "m")
        .with_affected(["b", "a"].into_iter().map(ElementId::from));
    assert_eq!(
        from_array.affected(),
        ids(&["b", "a"]).as_slice(),
        "LIB-DIAG-001: affected() from an array is wrong"
    );
    assert_eq!(
        from_iterator, from_array,
        "LIB-DIAG-001: affected() depends on the kind of iterator given"
    );
}

// --- 3. Severity ----------------------------------------------------------

#[test]
fn lib_diag_001_severity_orders_info_before_warning_before_error() {
    assert!(
        Severity::Info < Severity::Warning,
        "LIB-DIAG-001: Info is not below Warning"
    );
    assert!(
        Severity::Warning < Severity::Error,
        "LIB-DIAG-001: Warning is not below Error"
    );
    assert!(
        Severity::Info < Severity::Error,
        "LIB-DIAG-001: Info is not below Error"
    );
    let mut severities = vec![
        Severity::Error,
        Severity::Info,
        Severity::Warning,
        Severity::Info,
    ];
    severities.sort();
    assert_eq!(
        severities,
        [
            Severity::Info,
            Severity::Info,
            Severity::Warning,
            Severity::Error
        ],
        "LIB-DIAG-001: severities do not sort as Info < Warning < Error"
    );
    assert_eq!(
        Severity::Warning.cmp(&Severity::Warning),
        Ordering::Equal,
        "LIB-DIAG-001: a severity does not compare equal to itself"
    );
}

#[test]
fn lib_diag_001_severity_text_is_info_warning_error() {
    for (severity, text) in [
        (Severity::Info, "info"),
        (Severity::Warning, "warning"),
        (Severity::Error, "error"),
    ] {
        assert_eq!(
            severity.as_str(),
            text,
            "LIB-DIAG-001: {severity:?}.as_str() is wrong"
        );
        assert_eq!(
            severity.to_string(),
            text,
            "LIB-DIAG-001: {severity:?} displays wrongly"
        );
    }
}

// --- 4. Code --------------------------------------------------------------

/// Code texts, including ones a normalising implementation would change.
const CODE_TEXTS: [&str; 7] = [
    "SACM-REF-001",
    "sacm-ref-001",
    "",
    " SACM-ID-001 ",
    "SACM-ÜNÏ-標識",
    "a\0b",
    "X",
];

#[test]
fn lib_diag_001_code_as_str_and_display_return_the_text_unchanged() {
    for text in CODE_TEXTS {
        let code = Code::new(text);
        assert_eq!(
            code.as_str(),
            text,
            "LIB-DIAG-001: Code::new({text:?}).as_str() changed the text"
        );
        assert_eq!(
            code.to_string(),
            text,
            "LIB-DIAG-001: Code::new({text:?}) displays a changed text"
        );
    }
}

#[test]
fn lib_diag_001_codes_are_equal_exactly_when_their_text_is() {
    for a in CODE_TEXTS {
        for b in CODE_TEXTS {
            assert_eq!(
                Code::new(a) == Code::new(b),
                a == b,
                "LIB-DIAG-001: Code equality of {a:?} and {b:?} differs from the text's"
            );
        }
    }
}

#[test]
fn lib_diag_001_codes_order_by_their_text_not_its_length() {
    // "AA" is longer than "B" but earlier as text.
    assert_eq!(
        Code::new("AA").cmp(&Code::new("B")),
        Ordering::Less,
        "LIB-DIAG-001: Code AA does not order before Code B"
    );
    assert!(
        Code::new("AA") < Code::new("B"),
        "LIB-DIAG-001: Code AA is not below Code B"
    );
    assert_eq!(
        Code::new("B").cmp(&Code::new("AA")),
        Ordering::Greater,
        "LIB-DIAG-001: Code B does not order after Code AA"
    );
    let mut codes = vec![Code::new("B"), Code::new("AA")];
    codes.sort();
    assert_eq!(
        codes,
        [Code::new("AA"), Code::new("B")],
        "LIB-DIAG-001: codes do not sort by their text"
    );
}

// --- 5. SourceLocation ----------------------------------------------------

#[test]
fn lib_diag_001_source_location_refuses_a_zero_line_or_column() {
    for (line, column) in [(0, 0), (0, 1), (1, 0), (0, 5), (5, 0), (0, u32::MAX)] {
        assert_eq!(
            SourceLocation::new(line, column),
            None,
            "LIB-DIAG-001: SourceLocation::new({line}, {column}) accepted a 0 position"
        );
    }
}

#[test]
fn lib_diag_001_source_location_line_and_column_round_trip() {
    for (line, column) in [(1, 1), (3, 7), (7, 3), (1, u32::MAX), (u32::MAX, u32::MAX)] {
        let location = loc(line, column);
        assert_eq!(
            (location.line(), location.column()),
            (line, column),
            "LIB-DIAG-001: SourceLocation::new({line}, {column}) does not round-trip"
        );
    }
}

#[test]
fn lib_diag_001_source_location_displays_line_then_column() {
    for (line, column, text) in [
        (1, 1, "line 1, column 1"),
        (3, 7, "line 3, column 7"),
        (120, 4, "line 120, column 4"),
        (u32::MAX, u32::MAX, "line 4294967295, column 4294967295"),
    ] {
        assert_eq!(
            loc(line, column).to_string(),
            text,
            "LIB-DIAG-001: SourceLocation ({line}, {column}) displays wrongly"
        );
    }
}

#[test]
fn lib_diag_001_source_location_orders_by_line_then_column() {
    // Shuffled; the column of an earlier line is larger than the next line's.
    let mut locations = vec![loc(2, 1), loc(1, 9), loc(3, 1), loc(2, 2), loc(1, 1)];
    locations.sort();
    assert_eq!(
        locations,
        [loc(1, 1), loc(1, 9), loc(2, 1), loc(2, 2), loc(3, 1)],
        "LIB-DIAG-001: source locations do not sort by line, then column"
    );
    assert!(
        loc(1, u32::MAX) < loc(2, 1),
        "LIB-DIAG-001: a later column outranks an earlier line"
    );
    assert_eq!(
        loc(4, 4).cmp(&loc(4, 4)),
        Ordering::Equal,
        "LIB-DIAG-001: equal source locations do not compare equal"
    );
}

// --- 6. Display -----------------------------------------------------------

#[test]
fn lib_diag_001_display_with_clause_and_location() {
    let diagnostic = Diagnostic::new(
        Code::new("SACM-REF-001"),
        Severity::Error,
        "SACM23-REF-001",
        "the target \"claim_9\" does not resolve",
    )
    .with_clause("11.13")
    .with_location(loc(12, 34));
    assert_eq!(
        diagnostic.to_string(),
        "error SACM-REF-001 [SACM23-REF-001, clause 11.13] at line 12, column 34: \
         the target \"claim_9\" does not resolve",
        "LIB-DIAG-001: Display with a clause and a location is wrong"
    );
}

#[test]
fn lib_diag_001_display_with_clause_without_location() {
    let diagnostic = Diagnostic::new(
        Code::new("SACM-MULT-001"),
        Severity::Error,
        "SACM23-ARG-007",
        "an asserted relationship needs exactly one target",
    )
    .with_clause("11.13");
    assert_eq!(
        diagnostic.to_string(),
        "error SACM-MULT-001 [SACM23-ARG-007, clause 11.13]: \
         an asserted relationship needs exactly one target",
        "LIB-DIAG-001: Display with a clause and no location is wrong"
    );
}

#[test]
fn lib_diag_001_display_with_location_without_clause() {
    let diagnostic = Diagnostic::new(
        Code::new("SACM-XMI-003"),
        Severity::Warning,
        "SACM23-XMI-003",
        "the element is not SACM and is kept as it is",
    )
    .with_location(loc(1, 1));
    assert_eq!(
        diagnostic.to_string(),
        "warning SACM-XMI-003 [SACM23-XMI-003] at line 1, column 1: \
         the element is not SACM and is kept as it is",
        "LIB-DIAG-001: Display with a location and no clause is wrong"
    );
}

#[test]
fn lib_diag_001_display_without_clause_or_location() {
    let diagnostic = Diagnostic::new(
        Code::new("SACM-XMI-008"),
        Severity::Info,
        "SACM23-XMI-008",
        "loaded in compatibility mode",
    );
    assert_eq!(
        diagnostic.to_string(),
        "info SACM-XMI-008 [SACM23-XMI-008]: loaded in compatibility mode",
        "LIB-DIAG-001: Display with neither a clause nor a location is wrong"
    );
}

#[test]
fn lib_diag_001_display_does_not_show_the_affected_ids() {
    // The ADR 0011 format has no place for the affected ids.
    let without = Diagnostic::new(Code::new("SACM-ID-001"), Severity::Error, "ROW-1", "dup")
        .with_clause("8.2")
        .with_location(loc(5, 6));
    let with = without
        .clone()
        .with_affected(ids(&["claim_top", "claim_sub"]));
    assert_eq!(
        with.to_string(),
        "error SACM-ID-001 [ROW-1, clause 8.2] at line 5, column 6: dup",
        "LIB-DIAG-001: Display with affected ids is wrong"
    );
    assert_eq!(
        with.to_string(),
        without.to_string(),
        "LIB-DIAG-001: affected ids changed the Display"
    );
}

#[test]
fn lib_diag_001_display_shows_every_severity_by_its_text() {
    for (severity, text) in [
        (Severity::Info, "info"),
        (Severity::Warning, "warning"),
        (Severity::Error, "error"),
    ] {
        let diagnostic = Diagnostic::new(Code::new("C-1"), severity, "ROW-1", "m");
        assert_eq!(
            diagnostic.to_string(),
            format!("{text} C-1 [ROW-1]: m"),
            "LIB-DIAG-001: Display of a {text} diagnostic is wrong"
        );
    }
}

// --- 7. Order -------------------------------------------------------------

/// The fields of a diagnostic, in ADR 0011 key order.
#[derive(Clone, Copy)]
struct Fields {
    location: Option<(u32, u32)>,
    code: &'static str,
    affected: &'static [&'static str],
    requirement_id: &'static str,
    clause: Option<&'static str>,
    severity: Severity,
    message: &'static str,
}

impl Fields {
    fn build(self) -> Diagnostic {
        let mut diagnostic = Diagnostic::new(
            Code::new(self.code),
            self.severity,
            self.requirement_id,
            self.message,
        )
        .with_affected(ids(self.affected));
        if let Some(clause) = self.clause {
            diagnostic = diagnostic.with_clause(clause);
        }
        if let Some((line, column)) = self.location {
            diagnostic = diagnostic.with_location(loc(line, column));
        }
        diagnostic
    }
}

/// The diagnostic every variant differs from, in exactly one field.
const BASE: Fields = Fields {
    location: Some((5, 5)),
    code: "B",
    affected: &["b"],
    requirement_id: "R-B",
    clause: Some("2"),
    severity: Severity::Warning,
    message: "m-b",
};

/// Diagnostics that each differ from `BASE` in exactly one field, with
/// `BASE` itself, listed in the ADR 0011 order:
/// 1. location (located first, by line, then column);
/// 2. code; 3. affected (as a slice); 4. requirement id;
/// 5. clause (None first); 6. severity; 7. message.
///
/// Text keys compare as `str`, and affected ids id by id, so a longer value
/// can sort first: `AA` < `B`, `10` < `2`, `[a, z]` < `[b]`.
const SORTED: [(&str, Fields); 24] = [
    // An earlier line with a later column.
    (
        "location (4, 9)",
        Fields {
            location: Some((4, 9)),
            ..BASE
        },
    ),
    ("code A", Fields { code: "A", ..BASE }),
    // A longer code that is earlier as text.
    ("code AA", Fields { code: "AA", ..BASE }),
    (
        "affected []",
        Fields {
            affected: &[],
            ..BASE
        },
    ),
    (
        "affected [a]",
        Fields {
            affected: &["a"],
            ..BASE
        },
    ),
    // A longer slice whose first id is earlier.
    (
        "affected [a, z]",
        Fields {
            affected: &["a", "z"],
            ..BASE
        },
    ),
    (
        "requirement R-A",
        Fields {
            requirement_id: "R-A",
            ..BASE
        },
    ),
    // A longer requirement id that is earlier as text.
    (
        "requirement R-AA",
        Fields {
            requirement_id: "R-AA",
            ..BASE
        },
    ),
    (
        "clause None",
        Fields {
            clause: None,
            ..BASE
        },
    ),
    // Clauses are text, not numbers: "10" and "11.13" come before "2".
    (
        "clause 10",
        Fields {
            clause: Some("10"),
            ..BASE
        },
    ),
    (
        "clause 11.13",
        Fields {
            clause: Some("11.13"),
            ..BASE
        },
    ),
    (
        "severity Info",
        Fields {
            severity: Severity::Info,
            ..BASE
        },
    ),
    (
        "message m-a",
        Fields {
            message: "m-a",
            ..BASE
        },
    ),
    // A longer message that is earlier as text.
    (
        "message m-aa",
        Fields {
            message: "m-aa",
            ..BASE
        },
    ),
    ("base", BASE),
    (
        "message m-c",
        Fields {
            message: "m-c",
            ..BASE
        },
    ),
    (
        "severity Error",
        Fields {
            severity: Severity::Error,
            ..BASE
        },
    ),
    (
        "clause 3",
        Fields {
            clause: Some("3"),
            ..BASE
        },
    ),
    // A shorter clause that is later as text than "11.13".
    (
        "clause 8.2",
        Fields {
            clause: Some("8.2"),
            ..BASE
        },
    ),
    (
        "requirement R-C",
        Fields {
            requirement_id: "R-C",
            ..BASE
        },
    ),
    // A longer slice with the same first id sorts after it.
    (
        "affected [b, a]",
        Fields {
            affected: &["b", "a"],
            ..BASE
        },
    ),
    ("code C", Fields { code: "C", ..BASE }),
    (
        "location (5, 6)",
        Fields {
            location: Some((5, 6)),
            ..BASE
        },
    ),
    // Unlocated diagnostics come after every located one.
    (
        "location None",
        Fields {
            location: None,
            ..BASE
        },
    ),
];

fn sorted_set() -> Vec<Diagnostic> {
    SORTED.iter().map(|(_, fields)| fields.build()).collect()
}

/// The names of `diagnostics`, as listed in `SORTED`.
fn names(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
    let expected = sorted_set();
    diagnostics
        .iter()
        .map(|diagnostic| {
            expected
                .iter()
                .position(|candidate| candidate == diagnostic)
                .map_or("<not in SORTED>", |index| SORTED[index].0)
        })
        .collect()
}

/// A deterministic Fisher-Yates shuffle driven by xorshift64.
fn shuffled<T: Clone>(items: &[T], seed: u64) -> Vec<T> {
    let mut items = items.to_vec();
    let mut state = seed;
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let bound = u64::try_from(i + 1).unwrap_or(u64::MAX);
        let j = usize::try_from(state % bound).unwrap_or(0);
        items.swap(i, j);
    }
    items
}

/// Named permutations of the sorted set: identity, reversed, rotations,
/// interleavings and seeded shuffles.
fn permutations() -> Vec<(String, Vec<Diagnostic>)> {
    let sorted = sorted_set();
    let mut permutations = vec![
        ("sorted".to_owned(), sorted.clone()),
        (
            "reversed".to_owned(),
            sorted.iter().rev().cloned().collect(),
        ),
    ];
    for shift in [1, 8, 16] {
        let mut rotated = sorted.clone();
        rotated.rotate_left(shift);
        permutations.push((format!("rotated left by {shift}"), rotated));
    }
    let (even, odd): (Vec<_>, Vec<_>) = sorted
        .iter()
        .cloned()
        .enumerate()
        .partition(|(index, _)| index % 2 == 0);
    permutations.push((
        "odd positions reversed, then even positions".to_owned(),
        odd.into_iter()
            .rev()
            .chain(even)
            .map(|(_, diagnostic)| diagnostic)
            .collect(),
    ));
    for seed in [1, 0x9E37_79B9_7F4A_7C15, 42, 2026, 0xDEAD_BEEF] {
        permutations.push((
            format!("shuffled with seed {seed}"),
            shuffled(&sorted, seed),
        ));
    }
    permutations
}

#[test]
fn lib_diag_001_sorting_any_permutation_gives_the_key_order() {
    let expected: Vec<&str> = SORTED.iter().map(|(name, _)| *name).collect();
    for (how, mut diagnostics) in permutations() {
        diagnostics.sort();
        assert_eq!(
            names(&diagnostics),
            expected,
            "LIB-DIAG-001: sorting the {how} permutation does not give the ADR 0011 order"
        );
        diagnostics.reverse();
        diagnostics.sort_unstable();
        assert_eq!(
            names(&diagnostics),
            expected,
            "LIB-DIAG-001: an unstable sort of the {how} permutation does not give the ADR 0011 order"
        );
    }
}

#[test]
fn lib_diag_001_each_adjacent_pair_of_the_sorted_set_is_strictly_ordered() {
    let sorted = sorted_set();
    for (index, pair) in sorted.windows(2).enumerate() {
        assert_eq!(
            pair[0].cmp(&pair[1]),
            Ordering::Less,
            "LIB-DIAG-001: {:?} does not sort before {:?}",
            SORTED[index].0,
            SORTED[index + 1].0
        );
    }
}

#[test]
fn lib_diag_001_located_diagnostics_come_before_unlocated_ones() {
    // Every other field of the unlocated diagnostic sorts earlier.
    let unlocated = Fields {
        location: None,
        code: "A",
        affected: &[],
        requirement_id: "R-A",
        clause: None,
        severity: Severity::Info,
        message: "m-a",
    }
    .build();
    for (line, column) in [(1, 1), (5, 5), (u32::MAX, u32::MAX)] {
        let located = Fields {
            location: Some((line, column)),
            code: "Z",
            affected: &["z"],
            requirement_id: "R-Z",
            clause: Some("9"),
            severity: Severity::Error,
            message: "m-z",
        }
        .build();
        assert_eq!(
            located.cmp(&unlocated),
            Ordering::Less,
            "LIB-DIAG-001: a diagnostic at ({line}, {column}) does not sort before an unlocated one"
        );
        assert_eq!(
            unlocated.cmp(&located),
            Ordering::Greater,
            "LIB-DIAG-001: an unlocated diagnostic does not sort after one at ({line}, {column})"
        );
    }
}

/// Cases that are earlier than `BASE` in one key field and later in the next,
/// or later in one and earlier in the next. The earlier field decides.
const PRIORITY_CASES: [(&str, Fields, Ordering); 14] = [
    (
        "earlier location, later code",
        Fields {
            location: Some((4, 5)),
            code: "C",
            ..BASE
        },
        Ordering::Less,
    ),
    (
        "unlocated, earlier code",
        Fields {
            location: None,
            code: "A",
            ..BASE
        },
        Ordering::Greater,
    ),
    (
        "earlier line, later column",
        Fields {
            location: Some((4, 9)),
            ..BASE
        },
        Ordering::Less,
    ),
    (
        "later column, earlier code",
        Fields {
            location: Some((5, 6)),
            code: "A",
            ..BASE
        },
        Ordering::Greater,
    ),
    (
        "earlier code, later affected",
        Fields {
            code: "A",
            affected: &["c"],
            ..BASE
        },
        Ordering::Less,
    ),
    (
        "later code, earlier affected",
        Fields {
            code: "C",
            affected: &["a"],
            ..BASE
        },
        Ordering::Greater,
    ),
    (
        "earlier affected, later requirement",
        Fields {
            affected: &["a"],
            requirement_id: "R-C",
            ..BASE
        },
        Ordering::Less,
    ),
    (
        "later affected, earlier requirement",
        Fields {
            affected: &["c"],
            requirement_id: "R-A",
            ..BASE
        },
        Ordering::Greater,
    ),
    (
        "earlier requirement, later clause",
        Fields {
            requirement_id: "R-A",
            clause: Some("3"),
            ..BASE
        },
        Ordering::Less,
    ),
    (
        "later requirement, no clause",
        Fields {
            requirement_id: "R-C",
            clause: None,
            ..BASE
        },
        Ordering::Greater,
    ),
    (
        "no clause, later severity",
        Fields {
            clause: None,
            severity: Severity::Error,
            ..BASE
        },
        Ordering::Less,
    ),
    (
        "later clause, earlier severity",
        Fields {
            clause: Some("3"),
            severity: Severity::Info,
            ..BASE
        },
        Ordering::Greater,
    ),
    (
        "earlier severity, later message",
        Fields {
            severity: Severity::Info,
            message: "m-c",
            ..BASE
        },
        Ordering::Less,
    ),
    (
        "later severity, earlier message",
        Fields {
            severity: Severity::Error,
            message: "m-a",
            ..BASE
        },
        Ordering::Greater,
    ),
];

#[test]
fn lib_diag_001_an_earlier_key_field_outranks_every_later_one() {
    let base = BASE.build();
    for (case, fields, ordering) in PRIORITY_CASES {
        let diagnostic = fields.build();
        assert_eq!(
            diagnostic.cmp(&base),
            ordering,
            "LIB-DIAG-001: {case}: compared to the base, the wrong key field decided"
        );
        assert_eq!(
            base.cmp(&diagnostic),
            ordering.reverse(),
            "LIB-DIAG-001: {case}: the order is not antisymmetric"
        );
    }
}

/// Pairs that differ in one key field only, earlier first, where the earlier
/// value is the longer one. Comparing by length first would reverse every pair.
/// The affected pairs cover both the slice's length and each id's length.
const LONGER_BUT_EARLIER: [(&str, Fields, Fields); 7] = [
    (
        "code AA, code B",
        Fields { code: "AA", ..BASE },
        Fields { code: "B", ..BASE },
    ),
    (
        "affected [a, z], affected [b]",
        Fields {
            affected: &["a", "z"],
            ..BASE
        },
        Fields {
            affected: &["b"],
            ..BASE
        },
    ),
    (
        "affected [aa], affected [b]",
        Fields {
            affected: &["aa"],
            ..BASE
        },
        Fields {
            affected: &["b"],
            ..BASE
        },
    ),
    (
        "requirement R-AA, requirement R-B",
        Fields {
            requirement_id: "R-AA",
            ..BASE
        },
        Fields {
            requirement_id: "R-B",
            ..BASE
        },
    ),
    (
        "clause 10, clause 2",
        Fields {
            clause: Some("10"),
            ..BASE
        },
        Fields {
            clause: Some("2"),
            ..BASE
        },
    ),
    (
        "clause 11.13, clause 8.2",
        Fields {
            clause: Some("11.13"),
            ..BASE
        },
        Fields {
            clause: Some("8.2"),
            ..BASE
        },
    ),
    (
        "message m-aa, message m-b",
        Fields {
            message: "m-aa",
            ..BASE
        },
        Fields {
            message: "m-b",
            ..BASE
        },
    ),
];

#[test]
fn lib_diag_001_text_keys_compare_as_str_and_affected_ids_id_by_id() {
    for (case, earlier, later) in LONGER_BUT_EARLIER {
        let (earlier, later) = (earlier.build(), later.build());
        assert_eq!(
            earlier.cmp(&later),
            Ordering::Less,
            "LIB-DIAG-001: {case}: the first does not compare before the second"
        );
        assert_eq!(
            later.cmp(&earlier),
            Ordering::Greater,
            "LIB-DIAG-001: {case}: the second does not compare after the first"
        );
        let mut diagnostics = vec![later.clone(), earlier.clone()];
        diagnostics.sort();
        assert_eq!(
            diagnostics,
            [earlier, later],
            "LIB-DIAG-001: {case}: sorting does not put the first before the second"
        );
    }
}

#[test]
fn lib_diag_001_cmp_is_equal_exactly_when_the_diagnostics_are_equal() {
    let mut diagnostics = sorted_set();
    // A separately built copy of the base, and a clone of the first.
    diagnostics.push(BASE.build());
    diagnostics.push(diagnostics[0].clone());
    for a in &diagnostics {
        for b in &diagnostics {
            assert_eq!(
                a.cmp(b) == Ordering::Equal,
                a == b,
                "LIB-DIAG-001: cmp and == disagree on {a} and {b}"
            );
            assert_eq!(
                a.partial_cmp(b),
                Some(a.cmp(b)),
                "LIB-DIAG-001: partial_cmp and cmp disagree on {a} and {b}"
            );
            assert_eq!(
                a.cmp(b),
                b.cmp(a).reverse(),
                "LIB-DIAG-001: the order of {a} and {b} is not antisymmetric"
            );
        }
    }
    assert_eq!(
        BASE.build(),
        BASE.build(),
        "LIB-DIAG-001: diagnostics built from the same fields are not equal"
    );
}

#[test]
fn lib_diag_001_every_key_field_takes_part_in_equality() {
    let base = BASE.build();
    for (name, fields) in &SORTED[..] {
        if *name == "base" {
            continue;
        }
        assert_ne!(
            fields.build(),
            base,
            "LIB-DIAG-001: a diagnostic differing only in {name} equals the base"
        );
    }
}

// --- 8. Traits ------------------------------------------------------------

fn assert_value<T: Clone + Debug + Eq + Hash + Ord + Send + Sync + 'static>() {}
fn assert_copy_value<T: Copy + Debug + Eq + Hash + Ord + Send + Sync + 'static>() {}

#[test]
fn lib_diag_001_diagnostic_types_have_the_value_traits() {
    assert_value::<Diagnostic>();
    assert_copy_value::<Severity>();
    assert_copy_value::<Code>();
    assert_copy_value::<SourceLocation>();
}

#[test]
fn lib_diag_001_equal_diagnostics_hash_equally() {
    let a = BASE.build();
    let b = BASE.build();
    assert_eq!(
        hash_of(&a),
        hash_of(&b),
        "LIB-DIAG-001: equal diagnostics hash differently"
    );
    let mut set: HashSet<Diagnostic> = sorted_set().into_iter().collect();
    assert_eq!(
        set.len(),
        SORTED.len(),
        "LIB-DIAG-001: distinct diagnostics collapsed in a HashSet"
    );
    assert!(
        !set.insert(b),
        "LIB-DIAG-001: a HashSet took an equal diagnostic twice"
    );
}

#[test]
fn lib_diag_001_clone_is_equal_and_debug_shows_the_message() {
    let diagnostic = Fields {
        affected: &["claim_top", "claim_sub"],
        ..BASE
    }
    .build();
    let copy = diagnostic.clone();
    assert_eq!(
        copy, diagnostic,
        "LIB-DIAG-001: a cloned diagnostic differs from its original"
    );
    let debug = format!("{diagnostic:?}");
    assert!(
        debug.contains("m-b"),
        "LIB-DIAG-001: Debug of a diagnostic hides its message: {debug}"
    );
}
