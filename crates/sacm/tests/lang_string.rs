//! Tests for the `LangString` value type in `sacm::model` (SACM23-BASE-001,
//! formal/23-05-08 §8.3, ADR 0012).
//!
//! `LangString` has an optional `lang` and an optional `content`. For each
//! field, absent (`None`), empty (`Some("")`) and non-empty are three distinct
//! values, and both fields are kept byte for byte.
//!
//! Not checked here:
//! - Item 7's "no `id()` or `kind()` method": a test cannot show that a method
//!   is absent. The verifier checks the source and the rustdoc item list. The
//!   part a test can check, that no `ElementKind` or `AbstractClass` is named
//!   `LangString`, is checked below.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use sacm::model::{AbstractClass, ElementKind, LangString};

/// One mebibyte.
const MIB: usize = 1 << 20;

/// The three states of one field: absent, empty and non-empty.
const STATES: [Option<&str>; 3] = [None, Some(""), Some("x")];

/// The nine `(lang, content)` combinations of `STATES`, in the expected
/// `Ord` order: `lang` first, then `content`, absent before present.
fn nine() -> Vec<(Option<&'static str>, Option<&'static str>)> {
    let mut combinations = Vec::new();
    for lang in STATES {
        for content in STATES {
            combinations.push((lang, content));
        }
    }
    combinations
}

/// Builds a value with the setters, starting from the default.
fn with_setters(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut value = LangString::default();
    value.set_lang(lang.map(str::to_owned));
    value.set_content(content.map(str::to_owned));
    value
}

/// Builds a value with `default`, `new` and `with_lang` only.
fn with_constructors(lang: Option<&str>, content: Option<&str>) -> LangString {
    let value = match content {
        None => LangString::default(),
        Some(content) => LangString::new(content),
    };
    match lang {
        None => value,
        Some(lang) => value.with_lang(lang),
    }
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Content that a trimming, folding or normalising implementation would
/// change, or that a careless one would cut short.
const HOSTILE: [&str; 21] = [
    " leading space",
    "trailing space ",
    "  both  ",
    " ",
    "line one\nline two",
    "\n",
    "trailing LF\n",
    "line one\r\nline two",
    "lone\rCR",
    "\r",
    "tab\there",
    "\t",
    "\ttab first",
    "Säkerhetsfall är ÅÄÖ",
    "安全論証",
    "\u{1F6A6} traffic light",
    "Mixed CASE",
    "\u{e9}",
    "e\u{301}",
    "a\u{0}b",
    "\u{0}",
];

// --- 1. Default, new and with_lang -----------------------------------------

#[test]
fn sacm23_base_001_default_has_both_fields_absent() {
    let value = LangString::default();
    assert_eq!(
        value.lang(),
        None,
        "SACM23-BASE-001: LangString::default() has a lang"
    );
    assert_eq!(
        value.content(),
        None,
        "SACM23-BASE-001: LangString::default() has a content"
    );
}

#[test]
fn sacm23_base_001_new_sets_content_and_leaves_lang_absent() {
    for content in ["x", "", "Top claim"] {
        let from_str = LangString::new(content);
        assert_eq!(
            (from_str.lang(), from_str.content()),
            (None, Some(content)),
            "SACM23-BASE-001: LangString::new({content:?}) from &str is wrong"
        );
        let from_string = LangString::new(String::from(content));
        assert_eq!(
            from_string, from_str,
            "SACM23-BASE-001: LangString::new({content:?}) depends on String or &str"
        );
    }
}

#[test]
fn sacm23_base_001_new_with_empty_content_is_not_the_default() {
    assert_ne!(
        LangString::new(""),
        LangString::default(),
        "SACM23-BASE-001: LangString::new(\"\") equals the default, so empty content became absent"
    );
}

#[test]
fn sacm23_base_001_with_lang_sets_lang_and_keeps_content() {
    for lang in ["en", "", "sv-SE"] {
        let on_default = LangString::default().with_lang(lang);
        assert_eq!(
            (on_default.lang(), on_default.content()),
            (Some(lang), None),
            "SACM23-BASE-001: default().with_lang({lang:?}) is wrong"
        );
        let on_new = LangString::new("Top claim").with_lang(lang);
        assert_eq!(
            (on_new.lang(), on_new.content()),
            (Some(lang), Some("Top claim")),
            "SACM23-BASE-001: new(\"Top claim\").with_lang({lang:?}) is wrong"
        );
        let from_string = LangString::new("Top claim").with_lang(String::from(lang));
        assert_eq!(
            from_string, on_new,
            "SACM23-BASE-001: with_lang({lang:?}) depends on String or &str"
        );
    }
}

#[test]
fn sacm23_base_001_with_lang_twice_keeps_the_last_lang() {
    let value = LangString::new("c").with_lang("en").with_lang("de");
    assert_eq!(
        (value.lang(), value.content()),
        (Some("de"), Some("c")),
        "SACM23-BASE-001: a second with_lang did not replace the first"
    );
}

// --- 2. The nine combinations ---------------------------------------------

#[test]
fn sacm23_base_001_all_nine_combinations_read_back_exactly() {
    for (lang, content) in nine() {
        for (how, value) in [
            ("setters", with_setters(lang, content)),
            ("constructors", with_constructors(lang, content)),
        ] {
            assert_eq!(
                value.lang(),
                lang,
                "SACM23-BASE-001: lang() built with {how} from ({lang:?}, {content:?}) is wrong"
            );
            assert_eq!(
                value.content(),
                content,
                "SACM23-BASE-001: content() built with {how} from ({lang:?}, {content:?}) is wrong"
            );
        }
    }
}

#[test]
fn sacm23_base_001_constructors_and_setters_build_the_same_nine_values() {
    for (lang, content) in nine() {
        assert_eq!(
            with_constructors(lang, content),
            with_setters(lang, content),
            "SACM23-BASE-001: ({lang:?}, {content:?}) differs between constructors and setters"
        );
    }
}

#[test]
fn sacm23_base_001_the_nine_combinations_are_pairwise_unequal() {
    let combinations = nine();
    for (i, &(lang_a, content_a)) in combinations.iter().enumerate() {
        for (j, &(lang_b, content_b)) in combinations.iter().enumerate() {
            let a = with_setters(lang_a, content_a);
            let b = with_setters(lang_b, content_b);
            assert_eq!(
                a == b,
                i == j,
                "SACM23-BASE-001: ({lang_a:?}, {content_a:?}) == ({lang_b:?}, {content_b:?}) is wrong"
            );
        }
    }
}

#[test]
fn sacm23_base_001_absent_and_empty_are_distinct_in_each_field() {
    assert_ne!(
        LangString::default().with_lang(""),
        LangString::default(),
        "SACM23-BASE-001: an empty lang equals an absent lang"
    );
    assert_ne!(
        LangString::new("c").with_lang(""),
        LangString::new("c"),
        "SACM23-BASE-001: an empty lang equals an absent lang when content is set"
    );
    assert_ne!(
        LangString::new(""),
        LangString::default(),
        "SACM23-BASE-001: an empty content equals an absent content"
    );
    assert_ne!(
        LangString::new("").with_lang("en"),
        LangString::default().with_lang("en"),
        "SACM23-BASE-001: an empty content equals an absent content when lang is set"
    );
}

// --- 3. Content is kept byte for byte -------------------------------------

#[test]
fn sacm23_base_001_content_is_kept_byte_for_byte() {
    for content in HOSTILE {
        let built = [
            ("new", LangString::new(content)),
            ("new + with_lang", LangString::new(content).with_lang("en")),
            ("set_content", with_setters(None, Some(content))),
        ];
        for (how, value) in built {
            assert_eq!(
                value.content().map(str::as_bytes),
                Some(content.as_bytes()),
                "SACM23-BASE-001: content {content:?} built with {how} changed"
            );
            assert_eq!(
                value.clone().content().map(str::as_bytes),
                Some(content.as_bytes()),
                "SACM23-BASE-001: a clone of content {content:?} built with {how} changed"
            );
        }
    }
}

#[test]
fn sacm23_base_001_hostile_contents_are_pairwise_unequal() {
    for (i, a) in HOSTILE.iter().enumerate() {
        for (j, b) in HOSTILE.iter().enumerate() {
            assert_eq!(
                LangString::new(*a) == LangString::new(*b),
                i == j,
                "SACM23-BASE-001: contents {a:?} and {b:?} compare wrongly"
            );
        }
    }
}

#[test]
fn sacm23_base_001_nfc_and_nfd_compare_unequal() {
    // U+00E9 (NFC) and "e" + U+0301 COMBINING ACUTE ACCENT (NFD) are the same
    // text under Unicode normalisation, and different bytes.
    let nfc = "Caf\u{e9}";
    let nfd = "Cafe\u{301}";
    assert_ne!(
        nfc.as_bytes(),
        nfd.as_bytes(),
        "SACM23-BASE-001: the test's NFC and NFD forms have the same bytes"
    );
    assert_ne!(
        LangString::new(nfc),
        LangString::new(nfd),
        "SACM23-BASE-001: NFC and NFD content compare equal"
    );
    assert_eq!(
        LangString::new(nfd).content(),
        Some("Cafe\u{301}"),
        "SACM23-BASE-001: NFD content was normalised"
    );
    assert_eq!(
        LangString::new(nfc).content(),
        Some("Caf\u{e9}"),
        "SACM23-BASE-001: NFC content was normalised"
    );
    assert_ne!(
        LangString::default().with_lang(nfc),
        LangString::default().with_lang(nfd),
        "SACM23-BASE-001: NFC and NFD lang compare equal"
    );
}

#[test]
fn sacm23_base_001_content_differing_only_in_whitespace_or_case_compares_unequal() {
    let variants = ["x", " x", "x ", "x\n", "x\r\n", "x\r", "\tx", "X"];
    for a in variants {
        for b in variants {
            assert_eq!(
                LangString::new(a) == LangString::new(b),
                a == b,
                "SACM23-BASE-001: contents {a:?} and {b:?} compare wrongly"
            );
        }
    }
}

#[test]
fn sacm23_base_001_large_content_is_kept() {
    let content = "a\u{0}\u{e9}\n".repeat(MIB / 5);
    let value = LangString::new(content.as_str()).with_lang("en");
    assert_eq!(
        value.content().map(str::len),
        Some(content.len()),
        "SACM23-BASE-001: a large content changed length"
    );
    assert!(
        value.content() == Some(content.as_str()),
        "SACM23-BASE-001: a large content changed"
    );
}

// --- 4. lang has no format check ------------------------------------------

#[test]
fn sacm23_base_001_lang_takes_any_string_without_a_format_check() {
    for lang in [
        "",
        "en",
        "en-GB",
        "not a tag",
        "EN",
        " en ",
        "en_GB",
        "en\n",
        "\u{0}",
        "x-\u{e9}",
        "e\u{301}",
    ] {
        for (how, value) in [
            ("with_lang", LangString::default().with_lang(lang)),
            ("set_lang", with_setters(Some(lang), None)),
        ] {
            assert_eq!(
                value.lang().map(str::as_bytes),
                Some(lang.as_bytes()),
                "SACM23-BASE-001: lang {lang:?} set with {how} changed"
            );
        }
    }
}

#[test]
fn sacm23_base_001_lang_is_case_sensitive() {
    let upper = LangString::new("c").with_lang("en-GB");
    let lower = LangString::new("c").with_lang("en-gb");
    assert_ne!(
        upper, lower,
        "SACM23-BASE-001: lang en-GB and en-gb compare equal"
    );
    assert_eq!(
        upper.lang(),
        Some("en-GB"),
        "SACM23-BASE-001: lang en-GB was case-folded"
    );
    assert_eq!(
        lower.lang(),
        Some("en-gb"),
        "SACM23-BASE-001: lang en-gb was case-folded"
    );
}

// --- 5. Setters change only their own field -------------------------------

#[test]
fn sacm23_base_001_set_lang_changes_only_lang() {
    for (lang, content) in nine() {
        for new_lang in [Some("y"), Some(""), None] {
            let mut value = with_constructors(lang, content);
            value.set_lang(new_lang.map(str::to_owned));
            assert_eq!(
                (value.lang(), value.content()),
                (new_lang, content),
                "SACM23-BASE-001: set_lang({new_lang:?}) on ({lang:?}, {content:?}) is wrong"
            );
        }
    }
}

#[test]
fn sacm23_base_001_set_content_changes_only_content() {
    for (lang, content) in nine() {
        for new_content in [Some("y"), Some(""), None] {
            let mut value = with_constructors(lang, content);
            value.set_content(new_content.map(str::to_owned));
            assert_eq!(
                (value.lang(), value.content()),
                (lang, new_content),
                "SACM23-BASE-001: set_content({new_content:?}) on ({lang:?}, {content:?}) is wrong"
            );
        }
    }
}

#[test]
fn sacm23_base_001_setting_none_makes_a_field_absent_not_empty() {
    let mut value = LangString::new("c").with_lang("en");
    value.set_lang(None);
    value.set_content(None);
    assert_eq!(
        value,
        LangString::default(),
        "SACM23-BASE-001: clearing both fields does not give the default"
    );
    assert_eq!(
        (value.lang(), value.content()),
        (None, None),
        "SACM23-BASE-001: set_lang(None) or set_content(None) left a value"
    );
}

// --- 6. Order, hashing and traits -----------------------------------------

/// The nine values, built in the expected order.
fn sorted_nine() -> Vec<LangString> {
    nine()
        .into_iter()
        .map(|(lang, content)| with_setters(lang, content))
        .collect()
}

#[test]
fn sacm23_base_001_ord_compares_lang_then_content_absent_first() {
    let sorted = sorted_nine();
    for pair in sorted.windows(2) {
        assert_eq!(
            pair[0].cmp(&pair[1]),
            Ordering::Less,
            "SACM23-BASE-001: {:?} does not order before {:?}",
            pair[0],
            pair[1]
        );
    }
    let mut reversed: Vec<LangString> = sorted.iter().rev().cloned().collect();
    reversed.sort();
    assert_eq!(
        reversed, sorted,
        "SACM23-BASE-001: sorting the reversed nine does not give lang, then content, absent first"
    );
    let mut rotated = sorted.clone();
    rotated.rotate_left(4);
    rotated.sort_unstable();
    assert_eq!(
        rotated, sorted,
        "SACM23-BASE-001: an unstable sort of the rotated nine gives the wrong order"
    );
}

#[test]
fn sacm23_base_001_lang_outranks_content_in_the_order() {
    // Each pair: the first has the earlier lang and the later content.
    for (earlier, later) in [
        (with_setters(None, Some("x")), with_setters(Some(""), None)),
        (
            with_setters(Some(""), Some("x")),
            with_setters(Some("x"), None),
        ),
        (
            with_setters(Some("a"), Some("z")),
            with_setters(Some("b"), Some("a")),
        ),
    ] {
        assert_eq!(
            earlier.cmp(&later),
            Ordering::Less,
            "SACM23-BASE-001: content decided the order of {earlier:?} and {later:?}"
        );
        assert_eq!(
            later.cmp(&earlier),
            Ordering::Greater,
            "SACM23-BASE-001: the order of {later:?} and {earlier:?} is not antisymmetric"
        );
    }
}

#[test]
fn sacm23_base_001_text_compares_as_str_not_by_length() {
    // "AA" is longer than "B" but earlier as text, in either field.
    assert!(
        LangString::default().with_lang("AA") < LangString::default().with_lang("B"),
        "SACM23-BASE-001: lang AA does not order before lang B"
    );
    assert!(
        LangString::new("AA") < LangString::new("B"),
        "SACM23-BASE-001: content AA does not order before content B"
    );
}

#[test]
fn sacm23_base_001_cmp_is_equal_exactly_when_values_are_equal() {
    let mut values = sorted_nine();
    values.extend(HOSTILE.iter().map(|content| LangString::new(*content)));
    // Separately built copies.
    values.push(with_constructors(Some("x"), Some("")));
    values.push(LangString::new("\u{0}"));
    for a in &values {
        for b in &values {
            assert_eq!(
                a.cmp(b) == Ordering::Equal,
                a == b,
                "SACM23-BASE-001: cmp and == disagree on {a:?} and {b:?}"
            );
            assert_eq!(
                a.partial_cmp(b),
                Some(a.cmp(b)),
                "SACM23-BASE-001: partial_cmp and cmp disagree on {a:?} and {b:?}"
            );
            assert_eq!(
                a.cmp(b),
                b.cmp(a).reverse(),
                "SACM23-BASE-001: the order of {a:?} and {b:?} is not antisymmetric"
            );
        }
    }
}

#[test]
fn sacm23_base_001_equal_values_hash_equally() {
    for (lang, content) in nine() {
        assert_eq!(
            hash_of(&with_setters(lang, content)),
            hash_of(&with_constructors(lang, content)),
            "SACM23-BASE-001: equal values ({lang:?}, {content:?}) hash differently"
        );
    }
    for content in HOSTILE {
        let value = LangString::new(content).with_lang("en");
        assert_eq!(
            hash_of(&value),
            hash_of(&value.clone()),
            "SACM23-BASE-001: a clone of content {content:?} hashes differently"
        );
    }
    let mut set: HashSet<LangString> = sorted_nine().into_iter().collect();
    assert_eq!(
        set.len(),
        9,
        "SACM23-BASE-001: the nine combinations collapsed in a HashSet"
    );
    assert!(
        !set.insert(with_constructors(Some(""), None)),
        "SACM23-BASE-001: a HashSet took an equal value twice"
    );
}

fn assert_value_type<T: Clone + Debug + Default + Eq + Ord + Hash + Send + Sync + 'static>() {}

#[test]
fn sacm23_base_001_lang_string_has_the_value_traits() {
    assert_value_type::<LangString>();
}

#[test]
fn sacm23_base_001_clone_and_debug_work_on_every_value() {
    let mut values = sorted_nine();
    for content in HOSTILE {
        values.push(LangString::new(content).with_lang(content));
    }
    for value in values {
        let copy = value.clone();
        assert_eq!(
            copy, value,
            "SACM23-BASE-001: a clone differs from its original"
        );
        let debug = format!("{value:?}");
        assert!(
            !debug.is_empty(),
            "SACM23-BASE-001: Debug of {:?}/{:?} is empty",
            value.lang(),
            value.content()
        );
    }
}

// --- 7. Not a SACMElement --------------------------------------------------

#[test]
fn sacm23_base_001_no_element_kind_or_abstract_class_is_lang_string() {
    assert!(
        ElementKind::ALL
            .iter()
            .all(|kind| kind.name() != "LangString"),
        "SACM23-BASE-001: an ElementKind is named LangString"
    );
    assert!(
        AbstractClass::ALL
            .iter()
            .all(|class| class.name() != "LangString"),
        "SACM23-BASE-001: an AbstractClass is named LangString"
    );
}
