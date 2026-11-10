//! Tests for the `MultiLangString` value type in `sacm::model`
//! (SACM23-BASE-002, formal/23-05-08 §8.5).
//!
//! A `MultiLangString` is an ordered list of `LangString` values. It keeps the
//! order given, duplicates and duplicate languages, and may be empty in memory;
//! validation (SACM23-VAL-010) checks the lower bound and unique languages.
//!
//! Not checked here:
//! - Item 6's "no `id()` or `kind()` method", and the design's "no `Ord`": a
//!   test cannot show that a method or impl is absent. The verifier checks the
//!   source and the rustdoc item list. The part a test can check, that no
//!   `ElementKind` or `AbstractClass` is named `MultiLangString`, is below.

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use sacm::model::{AbstractClass, ElementKind, LangString, MultiLangString};

/// The three states of one field: absent, empty and non-empty.
const STATES: [Option<&str>; 3] = [None, Some(""), Some("x")];

fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut value = LangString::default();
    value.set_lang(lang.map(str::to_owned));
    value.set_content(content.map(str::to_owned));
    value
}

/// The nine absent/empty/non-empty `(lang, content)` combinations, reversed so
/// the sequence is not in `LangString`'s `Ord` order: a sorting implementation
/// would reorder it.
fn nine() -> Vec<LangString> {
    let mut values = Vec::new();
    for lang in STATES {
        for content in STATES {
            values.push(lang_string(lang, content));
        }
    }
    values.reverse();
    values
}

fn pushed(values: &[LangString]) -> MultiLangString {
    let mut multi = MultiLangString::default();
    for value in values {
        multi.push(value.clone());
    }
    multi
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn en() -> LangString {
    LangString::new("Top claim").with_lang("en")
}

fn fr() -> LangString {
    LangString::new("Revendication principale").with_lang("fr")
}

// --- 1. Default --------------------------------------------------------------

#[test]
fn sacm23_base_002_default_has_no_values() {
    let multi = MultiLangString::default();
    assert!(
        multi.values().is_empty(),
        "SACM23-BASE-002: MultiLangString::default() has values"
    );
    assert_eq!(
        multi.len(),
        0,
        "SACM23-BASE-002: MultiLangString::default() has a non-zero len"
    );
    assert!(
        multi.is_empty(),
        "SACM23-BASE-002: MultiLangString::default() is not empty"
    );
}

#[test]
fn sacm23_base_002_collecting_nothing_is_the_default_and_is_not_refused() {
    let collected: MultiLangString = std::iter::empty::<LangString>().collect();
    assert_eq!(
        collected,
        MultiLangString::default(),
        "SACM23-BASE-002: collecting no values differs from the default"
    );
    assert!(
        collected.is_empty(),
        "SACM23-BASE-002: collecting no values is not empty"
    );
}

// --- 2. Order and identity of values ----------------------------------------

#[test]
fn sacm23_base_002_collected_values_come_back_in_order_from_array_vec_and_iterator() {
    let given = nine();
    let array: [LangString; 9] = std::array::from_fn(|index| given[index].clone());
    let built = [
        ("an array", array.into_iter().collect::<MultiLangString>()),
        ("a Vec", given.clone().into_iter().collect()),
        ("an iterator", given.iter().cloned().collect()),
    ];
    for (how, multi) in built {
        let values: &[LangString] = multi.values();
        assert_eq!(
            values,
            given.as_slice(),
            "SACM23-BASE-002: values collected from {how} are not the inputs in order"
        );
        assert_eq!(
            multi.len(),
            given.len(),
            "SACM23-BASE-002: len() of values collected from {how} is wrong"
        );
        assert!(
            !multi.is_empty(),
            "SACM23-BASE-002: nine values collected from {how} are empty"
        );
    }
}

#[test]
fn sacm23_base_002_pushed_values_come_back_in_order() {
    let given = nine();
    let mut multi = MultiLangString::default();
    for (count, value) in given.iter().enumerate() {
        multi.push(value.clone());
        assert_eq!(
            multi.len(),
            count + 1,
            "SACM23-BASE-002: len() after {} pushes is wrong",
            count + 1
        );
        assert_eq!(
            multi.values(),
            &given[..=count],
            "SACM23-BASE-002: values after {} pushes are not the inputs in order",
            count + 1
        );
    }
    assert!(
        !multi.is_empty(),
        "SACM23-BASE-002: a MultiLangString with pushed values is empty"
    );
}

#[test]
fn sacm23_base_002_collecting_and_pushing_build_equal_values() {
    let given = nine();
    let collected: MultiLangString = given.iter().cloned().collect();
    assert_eq!(
        collected,
        pushed(&given),
        "SACM23-BASE-002: collecting and pushing the same values build unequal MultiLangStrings"
    );
    let mut extended = collected.clone();
    extended.push(en());
    let mut longer = given.clone();
    longer.push(en());
    assert_eq!(
        extended.values(),
        longer.as_slice(),
        "SACM23-BASE-002: push after collect did not append at the end"
    );
}

// --- 3. Duplicates -------------------------------------------------------------

#[test]
fn sacm23_base_002_duplicate_languages_and_duplicate_values_are_all_kept() {
    let cases = [
        (
            "two values with lang \"en\"",
            vec![
                LangString::new("a").with_lang("en"),
                LangString::new("b").with_lang("en"),
            ],
        ),
        (
            "two values with lang \"\"",
            vec![
                LangString::new("a").with_lang(""),
                LangString::new("b").with_lang(""),
            ],
        ),
        (
            "two values with lang absent",
            vec![LangString::new("a"), LangString::new("b")],
        ),
        ("two identical values", vec![en(), en()]),
        ("three identical defaults", vec![LangString::default(); 3]),
        ("en, fr, en", vec![en(), fr(), en()]),
    ];
    for (what, given) in cases {
        for (how, multi) in [
            ("collect", given.iter().cloned().collect()),
            ("push", pushed(&given)),
        ] {
            assert_eq!(
                multi.values(),
                given.as_slice(),
                "SACM23-BASE-002: {what} built with {how} were not all kept in order"
            );
            assert_eq!(
                multi.len(),
                given.len(),
                "SACM23-BASE-002: len() of {what} built with {how} does not count each one"
            );
        }
    }
}

#[test]
fn sacm23_base_002_push_never_overwrites_or_deduplicates() {
    let mut multi: MultiLangString = [en()].into_iter().collect();
    let second = LangString::new("Main claim").with_lang("en");
    multi.push(second.clone());
    multi.push(en());
    assert_eq!(
        multi.values(),
        [en(), second, en()].as_slice(),
        "SACM23-BASE-002: push replaced or dropped a value with the same lang or the same value"
    );
}

// --- 4. Equality and hashing -------------------------------------------------

#[test]
fn sacm23_base_002_equality_depends_on_order() {
    let en_fr: MultiLangString = [en(), fr()].into_iter().collect();
    let fr_en: MultiLangString = [fr(), en()].into_iter().collect();
    assert_ne!(
        en_fr, fr_en,
        "SACM23-BASE-002: [en, fr] equals [fr, en], so equality ignores order"
    );
    assert_eq!(
        en_fr,
        pushed(&[en(), fr()]),
        "SACM23-BASE-002: two [en, fr] built differently are unequal"
    );
}

#[test]
fn sacm23_base_002_absent_and_empty_lang_make_unequal_values() {
    let absent: MultiLangString = [LangString::new("c")].into_iter().collect();
    let empty: MultiLangString = [LangString::new("c").with_lang("")].into_iter().collect();
    assert_ne!(
        absent, empty,
        "SACM23-BASE-002: values that differ only in lang absent versus \"\" compare equal"
    );
}

#[test]
fn sacm23_base_002_empty_differs_from_one_default_value() {
    let one_default: MultiLangString = [LangString::default()].into_iter().collect();
    assert_ne!(
        MultiLangString::default(),
        one_default,
        "SACM23-BASE-002: an empty MultiLangString equals one holding LangString::default()"
    );
    assert_eq!(
        one_default.len(),
        1,
        "SACM23-BASE-002: a MultiLangString holding LangString::default() is not one value long"
    );
}

#[test]
fn sacm23_base_002_equal_values_hash_equally_and_unequal_ones_stay_apart() {
    let given = nine();
    let collected: MultiLangString = given.iter().cloned().collect();
    assert_eq!(
        hash_of(&collected),
        hash_of(&pushed(&given)),
        "SACM23-BASE-002: equal MultiLangStrings built differently hash differently"
    );
    assert_eq!(
        hash_of(&collected),
        hash_of(&collected.clone()),
        "SACM23-BASE-002: a clone hashes differently"
    );

    let distinct: Vec<MultiLangString> = vec![
        MultiLangString::default(),
        [LangString::default()].into_iter().collect(),
        [LangString::default(), LangString::default()]
            .into_iter()
            .collect(),
        [LangString::new("c")].into_iter().collect(),
        [LangString::new("c").with_lang("")].into_iter().collect(),
        [en(), fr()].into_iter().collect(),
        [fr(), en()].into_iter().collect(),
        collected.clone(),
    ];
    for (i, a) in distinct.iter().enumerate() {
        for (j, b) in distinct.iter().enumerate() {
            assert_eq!(a == b, i == j, "SACM23-BASE-002: {a:?} == {b:?} is wrong");
        }
    }
    let mut set: HashSet<MultiLangString> = distinct.iter().cloned().collect();
    assert_eq!(
        set.len(),
        distinct.len(),
        "SACM23-BASE-002: distinct MultiLangStrings collapsed in a HashSet"
    );
    assert!(
        !set.insert(pushed(&given)),
        "SACM23-BASE-002: a HashSet took an equal MultiLangString twice"
    );
}

// --- 5. Traits and no panics ---------------------------------------------------

fn assert_value_type<T: Clone + Debug + Default + Eq + Hash + Send + Sync + 'static>() {}

fn assert_from_iterator<T: FromIterator<LangString>>() {}

#[test]
fn sacm23_base_002_multi_lang_string_has_the_value_traits() {
    // Checked statically: this does not compile unless the bounds hold.
    assert_value_type::<MultiLangString>();
    assert_from_iterator::<MultiLangString>();
}

#[test]
fn sacm23_base_002_ten_thousand_values_build_clone_compare_hash_and_debug() {
    const COUNT: usize = 10_000;
    let given: Vec<LangString> = (0..COUNT)
        .map(|index| LangString::new(format!("name {index}")).with_lang(format!("x-{}", index % 7)))
        .collect();

    let collected: MultiLangString = given.iter().cloned().collect();
    let by_push = pushed(&given);
    assert_eq!(
        collected.len(),
        COUNT,
        "SACM23-BASE-002: 10,000 collected values have the wrong len"
    );
    assert_eq!(
        collected.values(),
        given.as_slice(),
        "SACM23-BASE-002: 10,000 collected values came back changed"
    );
    assert_eq!(
        collected, by_push,
        "SACM23-BASE-002: 10,000 values collected and pushed compare unequal"
    );

    let copy = collected.clone();
    drop(collected);
    assert_eq!(
        copy.values(),
        given.as_slice(),
        "SACM23-BASE-002: a clone of 10,000 values changed when the original was dropped"
    );
    assert_eq!(
        hash_of(&copy),
        hash_of(&by_push),
        "SACM23-BASE-002: 10,000 equal values hash differently"
    );

    let mut reordered = given.clone();
    reordered.swap(0, COUNT - 1);
    assert_ne!(
        copy,
        reordered.into_iter().collect::<MultiLangString>(),
        "SACM23-BASE-002: swapping the first and last of 10,000 values kept them equal"
    );

    let debug = format!("{copy:?}");
    assert!(
        debug.contains("name 0") && debug.contains(&format!("name {}", COUNT - 1)),
        "SACM23-BASE-002: Debug of 10,000 values lost values"
    );
}

#[test]
fn sacm23_base_002_hostile_values_are_kept_and_debug_does_not_panic() {
    let big = "a\u{0}\r\n\t".repeat((1 << 20) / 5);
    let given = vec![
        LangString::new(big.as_str()).with_lang("en"),
        LangString::new("\u{0}").with_lang("\u{0}"),
        LangString::new("line one\r\nline two\ttab\rCR"),
        LangString::new("Caf\u{e9}").with_lang("fr"),
        LangString::new("Cafe\u{301}").with_lang("fr"),
    ];
    let multi: MultiLangString = given.iter().cloned().collect();
    assert_eq!(
        multi.values(),
        given.as_slice(),
        "SACM23-BASE-002: hostile values came back changed"
    );
    assert_eq!(
        multi.len(),
        5,
        "SACM23-BASE-002: NFC and NFD values with the same lang collapsed"
    );
    assert!(
        format!("{multi:?}").len() > big.len(),
        "SACM23-BASE-002: Debug of a 1 MiB value does not show it"
    );
}

// --- 6. Not a SACMElement --------------------------------------------------------

#[test]
fn sacm23_base_002_no_element_kind_or_abstract_class_is_multi_lang_string() {
    assert!(
        ElementKind::ALL
            .iter()
            .all(|kind| kind.name() != "MultiLangString"),
        "SACM23-BASE-002: an ElementKind is named MultiLangString"
    );
    assert!(
        AbstractClass::ALL
            .iter()
            .all(|class| class.name() != "MultiLangString"),
        "SACM23-BASE-002: an AbstractClass is named MultiLangString"
    );
}
