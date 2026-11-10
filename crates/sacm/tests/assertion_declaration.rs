//! Tests for the `AssertionDeclaration` enumeration and its conversion error
//! `ParseAssertionDeclarationError` in `sacm::model` (SACM23-ARG-003,
//! formal/23-05-08 §11.8 and §11.10, ADR 0011 amendment 2).
//!
//! Not checked here:
//! - The `#[non_exhaustive]` parts of items 6 and 7: a test cannot show that an
//!   exhaustive `match`, or a struct literal of the error, fails to compile
//!   outside the crate. The verifier checks the attributes in the source and
//!   the rustdoc.
//! - Item 6's "private fields": a review item for the same reason.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::str::FromStr;

use sacm::model::{AssertionDeclaration, ParseAssertionDeclarationError};

/// One mebibyte.
const MIB: usize = 1 << 20;

/// The six literals in ptc/22-03-13 `ownedLiteral` order, with their spellings.
const LITERALS: [(AssertionDeclaration, &str); 6] = [
    (AssertionDeclaration::Asserted, "asserted"),
    (AssertionDeclaration::NeedsSupport, "needsSupport"),
    (AssertionDeclaration::Assumed, "assumed"),
    (AssertionDeclaration::Axiomatic, "axiomatic"),
    (AssertionDeclaration::Defeated, "defeated"),
    (AssertionDeclaration::AsCited, "asCited"),
];

/// The inputs acceptance item 5 lists, each of which must be refused.
const REJECTED: [&str; 8] = [
    "Asserted",
    "needssupport",
    " asserted",
    "asserted ",
    "",
    "justification",
    "byRule",
    "needs\nSupport",
];

/// The characters that break a line: `\n` and `\r`, NEL, LINE SEPARATOR and
/// PARAGRAPH SEPARATOR.
const LINE_BREAKS: [char; 5] = ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// Parses `input`, which the test expects to be refused, and returns the error.
fn refusal(input: &str) -> ParseAssertionDeclarationError {
    match input.parse::<AssertionDeclaration>() {
        Ok(literal) => {
            panic!("SACM23-ARG-003: {input:?} was accepted as {literal:?}")
        }
        Err(error) => error,
    }
}

// --- 1. ALL and Ord --------------------------------------------------------

#[test]
fn sacm23_arg_003_all_holds_the_six_literals_in_model_order() {
    let all: &'static [AssertionDeclaration] = AssertionDeclaration::ALL;
    assert_eq!(
        all,
        [
            AssertionDeclaration::Asserted,
            AssertionDeclaration::NeedsSupport,
            AssertionDeclaration::Assumed,
            AssertionDeclaration::Axiomatic,
            AssertionDeclaration::Defeated,
            AssertionDeclaration::AsCited,
        ],
        "SACM23-ARG-003: ALL is not the six literals in model order"
    );
}

#[test]
fn sacm23_arg_003_ord_follows_model_order() {
    for pair in AssertionDeclaration::ALL.windows(2) {
        assert_eq!(
            pair[0].cmp(&pair[1]),
            Ordering::Less,
            "SACM23-ARG-003: {:?} does not order before {:?}",
            pair[0],
            pair[1]
        );
    }
    let mut literals: Vec<AssertionDeclaration> =
        AssertionDeclaration::ALL.iter().rev().copied().collect();
    literals.sort();
    assert_eq!(
        literals,
        AssertionDeclaration::ALL,
        "SACM23-ARG-003: sorting the literals does not give model order"
    );
    for (i, a) in AssertionDeclaration::ALL.iter().enumerate() {
        for (j, b) in AssertionDeclaration::ALL.iter().enumerate() {
            assert_eq!(
                a.cmp(b),
                i.cmp(&j),
                "SACM23-ARG-003: {a:?} and {b:?} do not compare by model position"
            );
            assert_eq!(
                a.partial_cmp(b),
                Some(i.cmp(&j)),
                "SACM23-ARG-003: partial_cmp of {a:?} and {b:?} disagrees with cmp"
            );
            assert_eq!(a == b, i == j, "SACM23-ARG-003: {a:?} == {b:?} is wrong");
        }
    }
}

// --- 2. as_str and Display -------------------------------------------------

#[test]
fn sacm23_arg_003_as_str_and_display_give_the_exact_spellings() {
    for (literal, spelling) in LITERALS {
        assert_eq!(
            literal.as_str(),
            spelling,
            "SACM23-ARG-003: {literal:?}.as_str() is wrong"
        );
        assert_eq!(
            literal.to_string(),
            spelling,
            "SACM23-ARG-003: {literal:?} displays wrongly"
        );
    }
    let spellings: Vec<&'static str> = AssertionDeclaration::ALL
        .iter()
        .map(|literal| literal.as_str())
        .collect();
    assert_eq!(
        spellings,
        [
            "asserted",
            "needsSupport",
            "assumed",
            "axiomatic",
            "defeated",
            "asCited"
        ],
        "SACM23-ARG-003: the spellings of ALL are wrong or out of order"
    );
}

// --- 3. Round trip ---------------------------------------------------------

#[test]
fn sacm23_arg_003_parsing_each_spelling_gives_back_the_literal() {
    for &literal in AssertionDeclaration::ALL {
        assert_eq!(
            literal.as_str().parse::<AssertionDeclaration>(),
            Ok(literal),
            "SACM23-ARG-003: parsing {literal:?}.as_str() does not give it back"
        );
        assert_eq!(
            AssertionDeclaration::from_str(&literal.to_string()),
            Ok(literal),
            "SACM23-ARG-003: parsing the Display of {literal:?} does not give it back"
        );
    }
    for (literal, spelling) in LITERALS {
        assert_eq!(
            spelling.parse::<AssertionDeclaration>(),
            Ok(literal),
            "SACM23-ARG-003: parsing {spelling:?} does not give {literal:?}"
        );
    }
}

// --- 4. Default ------------------------------------------------------------

#[test]
fn sacm23_arg_003_default_is_asserted() {
    assert_eq!(
        AssertionDeclaration::default(),
        AssertionDeclaration::Asserted,
        "SACM23-ARG-003: the default is not Asserted"
    );
}

// --- 5. Rejected inputs ----------------------------------------------------

#[test]
fn sacm23_arg_003_rejects_the_listed_inputs_and_keeps_them_exactly() {
    for input in REJECTED {
        let error = refusal(input);
        assert_eq!(
            error.input(),
            input,
            "SACM23-ARG-003: input() of the error for {input:?} is not the input"
        );
        assert_eq!(
            error.input().as_bytes(),
            input.as_bytes(),
            "SACM23-ARG-003: input() of the error for {input:?} changed its bytes"
        );
    }
}

#[test]
fn sacm23_arg_003_needs_support_with_a_line_break_keeps_the_line_break() {
    let error = refusal("needs\nSupport");
    assert_eq!(
        error.input(),
        "needs\nSupport",
        "SACM23-ARG-003: input() lost or escaped the line break"
    );
}

#[test]
fn sacm23_arg_003_rejects_case_and_whitespace_variants_of_every_literal() {
    for (literal, spelling) in LITERALS {
        let spelling: &str = spelling;
        let mut first_upper = spelling[..1].to_uppercase();
        first_upper.push_str(&spelling[1..]);
        let variants: [String; 13] = [
            spelling.to_uppercase(),
            spelling.to_lowercase(),
            first_upper,
            format!(" {spelling}"),
            format!("{spelling} "),
            format!("\t{spelling}"),
            format!("{spelling}\n"),
            format!("{spelling}\r\n"),
            format!("{spelling}\u{0}"),
            format!("\u{feff}{spelling}"),
            format!("\"{spelling}\""),
            format!("{spelling}{spelling}"),
            spelling[..spelling.len() - 1].to_owned(),
        ];
        for variant in variants {
            if variant == spelling {
                // `to_lowercase` leaves the all-lowercase spellings unchanged.
                continue;
            }
            let error = refusal(&variant);
            assert_eq!(
                error.input(),
                variant,
                "SACM23-ARG-003: input() for the {literal:?} variant {variant:?} is wrong"
            );
        }
    }
}

#[test]
fn sacm23_arg_003_rejects_the_rust_variant_names() {
    for name in [
        "NeedsSupport",
        "AsCited",
        "Assumed",
        "Axiomatic",
        "Defeated",
    ] {
        let error = refusal(name);
        assert_eq!(
            error.input(),
            name,
            "SACM23-ARG-003: input() for the variant name {name:?} is wrong"
        );
    }
}

#[test]
fn sacm23_arg_003_rejects_a_large_input_without_panicking() {
    let input = "asserted".repeat(MIB / 8);
    let error = refusal(&input);
    assert!(
        error.input() == input,
        "SACM23-ARG-003: input() of a large rejected input changed"
    );
}

// --- 6. ParseAssertionDeclarationError -------------------------------------

fn assert_error_type<T: Clone + Eq + Debug + Display + Error + Send + Sync + 'static>() {}

#[test]
fn sacm23_arg_003_parse_error_has_the_error_traits() {
    assert_error_type::<ParseAssertionDeclarationError>();
    let boxed: Box<dyn Error + Send + Sync + 'static> = Box::new(refusal("Asserted"));
    assert_eq!(
        boxed.to_string(),
        "invalid AssertionDeclaration literal \"Asserted\"",
        "SACM23-ARG-003: the boxed error displays wrongly"
    );
}

#[test]
fn sacm23_arg_003_parse_error_message_is_exact() {
    for (input, message) in [
        (
            "Asserted",
            "invalid AssertionDeclaration literal \"Asserted\"",
        ),
        ("", "invalid AssertionDeclaration literal \"\""),
        (
            "justification",
            "invalid AssertionDeclaration literal \"justification\"",
        ),
        (
            " asserted",
            "invalid AssertionDeclaration literal \" asserted\"",
        ),
        (
            "needs\nSupport",
            "invalid AssertionDeclaration literal \"needs\\nSupport\"",
        ),
        (
            "say \"hi\"\\",
            "invalid AssertionDeclaration literal \"say \\\"hi\\\"\\\\\"",
        ),
        ("a\tb", "invalid AssertionDeclaration literal \"a\\tb\""),
        ("a\u{0}b", "invalid AssertionDeclaration literal \"a\\0b\""),
    ] {
        assert_eq!(
            refusal(input).to_string(),
            message,
            "SACM23-ARG-003: the message for {input:?} is wrong"
        );
    }
}

#[test]
fn sacm23_arg_003_parse_error_message_quotes_the_input_as_debug_does() {
    for input in REJECTED {
        assert_eq!(
            refusal(input).to_string(),
            format!("invalid AssertionDeclaration literal {input:?}"),
            "SACM23-ARG-003: the message for {input:?} does not quote it as {{:?}} does"
        );
    }
}

#[test]
fn sacm23_arg_003_parse_error_message_is_one_line() {
    for input in [
        "needs\nSupport",
        "a\rb",
        "a\r\nb",
        "a\u{85}b",
        "a\u{2028}b",
        "a\u{2029}b",
        "\n",
    ] {
        let message = refusal(input).to_string();
        assert!(
            !message.contains(LINE_BREAKS),
            "SACM23-ARG-003: the message for {input:?} contains a line break: {message:?}"
        );
        assert_eq!(
            message.lines().count(),
            1,
            "SACM23-ARG-003: the message for {input:?} is not one line: {message:?}"
        );
    }
}

#[test]
fn sacm23_arg_003_parse_error_message_starts_lowercase() {
    let message = refusal("Asserted").to_string();
    assert!(
        message.starts_with("invalid "),
        "SACM23-ARG-003: the message does not start with lowercase \"invalid \": {message:?}"
    );
    assert!(
        !message.ends_with('.'),
        "SACM23-ARG-003: the message ends with a full stop: {message:?}"
    );
}

#[test]
fn sacm23_arg_003_parse_error_has_no_source() {
    for input in REJECTED {
        assert!(
            refusal(input).source().is_none(),
            "SACM23-ARG-003: the error for {input:?} has a source"
        );
    }
}

#[test]
fn sacm23_arg_003_parse_errors_are_equal_exactly_when_their_inputs_are() {
    for a in REJECTED {
        for b in REJECTED {
            assert_eq!(
                refusal(a) == refusal(b),
                a == b,
                "SACM23-ARG-003: errors for {a:?} and {b:?} compare wrongly"
            );
        }
        let error = refusal(a);
        assert_eq!(
            error.clone(),
            error,
            "SACM23-ARG-003: a clone of the error for {a:?} differs"
        );
    }
}

// --- 7. AssertionDeclaration traits ----------------------------------------

fn assert_literal_type<
    T: Copy
        + Eq
        + Ord
        + Hash
        + Debug
        + Default
        + Display
        + FromStr<Err = ParseAssertionDeclarationError>
        + Send
        + Sync
        + 'static,
>() {
}

#[test]
fn sacm23_arg_003_assertion_declaration_has_the_value_traits() {
    assert_literal_type::<AssertionDeclaration>();
}

#[test]
fn sacm23_arg_003_the_six_literals_hash_apart() {
    let set: HashSet<AssertionDeclaration> = AssertionDeclaration::ALL.iter().copied().collect();
    assert_eq!(
        set.len(),
        6,
        "SACM23-ARG-003: the six literals collapsed in a HashSet"
    );
}
