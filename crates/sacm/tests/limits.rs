//! Read limits (LIB-SEC-003; the robustness policy of `docs/architecture.md`;
//! M6 decision Q9; ADR 0041 D1, D2, D3).
//!
//! `ReadOptions::with_limits(Limits)` bounds what `xmi::read` accepts, in
//! both modes: the input's size (32 MiB), the nesting depth of elements
//! (256), the XML nodes the parser builds (1,000,000) and the namespace
//! bindings in scope at any element (256). A breach is
//! `ReadError::LimitExceeded` (SACM-SEC-003), which names the limit, its value
//! and, where known, its location. Two counts need no setter (D2): the
//! bindings in scope at every element that declares one count against the
//! node limit, because roxmltree 0.21 copies them, and the namespace text
//! that preserved content keeps counts against the size limit: for each
//! preserved name the bytes of its namespace URI, and for each binding it
//! keeps `prefix + URI + 10` (the design brief of slice 0072, D2). The
//! parser's capacities that no limit covers stay `Unsupported` with
//! SACM-SEC-002 and no location.
//!
//! The default edges and 65,536 prefixes compare codes and templates by
//! their text. The peak memory of a read at the defaults was measured once,
//! outside the tests, and is recorded in the `xmi::read` docs ("# Limits").

use std::fmt::Write as _;

use sacm::diagnostic::SourceLocation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};
use sacm::xmi::{Limit, Limits, WriteOptions};

const ROW: &str = "LIB-SEC-003";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The start tag of a root ArgumentPackage `p`.
const ROOT_START: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"p\">",
);

const ROOT_END: &str = "</sacm:ArgumentPackage>";

/// The defaults: 32 MiB, depth 256, 1,000,000 nodes, 256 bindings.
const DEFAULT_INPUT_BYTES: usize = 33_554_432;
const DEFAULT_DEPTH: usize = 256;
const DEFAULT_BINDINGS: usize = 256;

// --- Helpers ---------------------------------------------------------------------------

fn options(mode: Mode) -> ReadOptions {
    ReadOptions::default().with_mode(mode)
}

fn loc(line: u32, column: u32) -> SourceLocation {
    SourceLocation::new(line, column)
        .unwrap_or_else(|| panic!("{ROW}: setup: ({line}, {column}) is no location"))
}

/// The location of the first `needle` in `text`, whose lines end with LF:
/// the line, and the column in characters.
fn position_of(text: &str, needle: &str) -> SourceLocation {
    nth_position_of(text, needle, 0)
}

/// The location of the `n`th (0-based) `needle` in `text`, as
/// [`position_of`].
fn nth_position_of(text: &str, needle: &str, n: usize) -> SourceLocation {
    let Some((offset, _)) = text.match_indices(needle).nth(n) else {
        panic!("{ROW}: setup: {needle:?} is not in the input {n} times and more");
    };
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    loc(
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}

/// The read of `input` with `options`, which must succeed.
fn read_ok(what: &str, input: &[u8], options: &ReadOptions) -> ReadOutput {
    match xmi::read(input, options) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: the read refuses it: {error:?} ({error})"),
    }
}

/// The refusal of `input` with `options`.
fn refusal(what: &str, input: &[u8], options: &ReadOptions) -> ReadError {
    match xmi::read(input, options) {
        Ok(output) => panic!(
            "{ROW}: {what}: the read returns a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// Asserts, with today's API only, that `input` read at the defaults in both
/// modes alike is refused with SACM-SEC-003, `template` and `location`.
fn assert_sec_003_at_the_defaults(
    what: &str,
    input: &[u8],
    template: &str,
    location: Option<SourceLocation>,
) {
    let strict = refusal(what, input, &options(Mode::Strict));
    let tolerant = refusal(what, input, &options(Mode::Tolerant));
    assert_eq!(
        (strict.code().as_str(), strict.template(), strict.location()),
        ("SACM-SEC-003", template, location),
        "{ROW}: {what}: the code, template and location of {strict}"
    );
    assert!(
        tolerant == strict,
        "{ROW}: {what}: the modes refuse differently: {tolerant:?} and {strict:?}"
    );
}

/// `options(mode)` under `limits`.
fn under(limits: Limits, mode: Mode) -> ReadOptions {
    options(mode).with_limits(limits)
}

/// Asserts that `input`, read under `limits`, reads in both modes.
fn assert_reads_under(what: &str, input: &[u8], limits: Limits) {
    for mode in MODES {
        read_ok(what, input, &under(limits, mode));
    }
}

/// Asserts that `input`, read under `limits` in both modes alike, is
/// `LimitExceeded` for `limit` with `max` at `location`, with the code
/// SACM-SEC-003 and `template`, and returns the refusal.
fn assert_exceeded(
    what: &str,
    input: &[u8],
    limits: Limits,
    expected: (Limit, u64, Option<SourceLocation>),
    template: &str,
) -> ReadError {
    let strict = refusal(what, input, &under(limits, Mode::Strict));
    let tolerant = refusal(what, input, &under(limits, Mode::Tolerant));
    assert_eq!(
        tolerant, strict,
        "{ROW}: {what}: the modes refuse differently"
    );
    let ReadError::LimitExceeded {
        limit,
        max,
        location,
        ..
    } = &strict
    else {
        panic!("{ROW}: {what}: not LimitExceeded: {strict:?}");
    };
    assert_eq!(
        (*limit, *max, *location),
        expected,
        "{ROW}: {what}: the limit, its value and the location"
    );
    assert_eq!(
        (strict.code().as_str(), strict.template(), strict.location()),
        ("SACM-SEC-003", template, expected.2),
        "{ROW}: {what}: code(), template() and location()"
    );
    strict
}

/// The arguments of `error` as `(name, count)`; a Text shows as `None`.
fn counts_of(error: &ReadError) -> Vec<(&'static str, Option<u64>)> {
    error
        .arguments()
        .iter()
        .map(|argument| (argument.name(), argument.value().as_count()))
        .collect()
}

/// A root ArgumentPackage `p` holding `body`.
fn package(body: &str) -> String {
    format!("{ROOT_START}{body}{ROOT_END}")
}

/// A root ArgumentPackage that binds `v` and nests `levels` preserved `v:x`
/// elements inside it: `levels + 1` deep.
fn chain(levels: usize) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:v\" xmi:id=\"p\">{}{}{ROOT_END}",
        "<v:x>".repeat(levels),
        "</v:x>".repeat(levels)
    )
}

/// A root ArgumentPackage that declares `extra` bindings `nN` beside `sacm`
/// and `xmi`: `extra + 2` in scope.
fn root_with_bindings(extra: usize) -> String {
    let mut input = format!("<sacm:ArgumentPackage{NS}");
    for n in 0..extra {
        let _ = write!(input, " xmlns:n{n}=\"urn:n\"");
    }
    input.push_str(" xmi:id=\"p\"/>");
    input
}

/// The design brief's kept-namespaces input: a pinned ArgumentPackage that
/// binds `v` to a 42-character URI and holds ten preserved `<v:x/>`. Each
/// keeps the binding (1 + 42 + 10) and the name's URI (42): 95 bytes.
fn ten_preserved() -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:a-namespace-of-forty-characters-long-x\" \
         xmi:id=\"p\">{}{ROOT_END}",
        "<v:x/>".repeat(10)
    )
}

/// The namespace text each `<v:x/>` of [`ten_preserved`] keeps.
const KEPT_PER_CHILD: usize = 1 + 42 + 10 + 42;

// --- 1. The type and its defaults ------------------------------------------------------

fn assert_traits<T: Copy + Send + Sync + Eq + std::hash::Hash + std::fmt::Debug>() {}

#[test]
fn lib_sec_003_limits_have_the_stated_defaults_and_setters() {
    let defaults = Limits::default();
    let values = |limits: Limits| {
        (
            limits.max_input_bytes(),
            limits.max_depth(),
            limits.max_nodes(),
            limits.max_namespace_bindings(),
        )
    };
    assert_eq!(
        values(defaults),
        (33_554_432, 256, 1_000_000, 256),
        "{ROW}: the defaults"
    );
    assert_eq!(
        [
            values(defaults.with_max_input_bytes(7)),
            values(defaults.with_max_depth(7)),
            values(defaults.with_max_nodes(7)),
            values(defaults.with_max_namespace_bindings(7)),
        ],
        [
            (7, 256, 1_000_000, 256),
            (33_554_432, 7, 1_000_000, 256),
            (33_554_432, 256, 7, 256),
            (33_554_432, 256, 1_000_000, 7),
        ],
        "{ROW}: each with_max_* sets only its own value"
    );
    assert_eq!(
        ReadOptions::default().limits(),
        Limits::default(),
        "{ROW}: ReadOptions::default() reads under the default limits"
    );
    let set = defaults
        .with_max_input_bytes(1)
        .with_max_depth(2)
        .with_max_nodes(3)
        .with_max_namespace_bindings(4);
    assert_eq!(values(set), (1, 2, 3, 4), "{ROW}: the setters chain");
    assert_eq!(
        ReadOptions::default().with_limits(set).limits(),
        set,
        "{ROW}: with_limits round-trips"
    );
    assert_eq!(
        ReadOptions::default()
            .with_limits(set)
            .with_mode(Mode::Strict)
            .limits(),
        set,
        "{ROW}: with_mode keeps the limits"
    );
    assert_eq!(
        ReadOptions::default()
            .with_mode(Mode::Strict)
            .with_limits(set)
            .mode(),
        Mode::Strict,
        "{ROW}: with_limits keeps the mode"
    );
    assert_traits::<Limits>();
    assert_traits::<Limit>();
    assert_eq!(
        [
            Limit::InputBytes.to_string(),
            Limit::Depth.to_string(),
            Limit::Nodes.to_string(),
            Limit::NamespaceBindings.to_string(),
        ],
        ["input bytes", "depth", "nodes", "namespace bindings"],
        "{ROW}: the Display of Limit"
    );
}

// --- 2. The input's size ---------------------------------------------------------------

#[test]
fn lib_sec_003_the_input_size_is_checked_first_on_the_bytes() {
    let input = package("");
    let n = input.len();
    let max = u64::try_from(n).unwrap_or(u64::MAX);
    let limits = Limits::default().with_max_input_bytes(max);
    // An input of n bytes reads under a limit of n.
    assert_reads_under("an input of n bytes", input.as_bytes(), limits);
    // One more byte, white space after the root, passes it.
    let longer = format!("{input} ");
    let error = assert_exceeded(
        "an input of n + 1 bytes",
        longer.as_bytes(),
        limits,
        (Limit::InputBytes, max, None),
        "SACM-SEC-003/input-bytes",
    );
    assert_eq!(
        counts_of(&error),
        [("size", Some(max + 1)), ("max", Some(max))],
        "{ROW}: the arguments size and max"
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the input is {} bytes long, over the size limit of {max} bytes",
            max + 1
        ),
        "{ROW}: the Display of the size refusal"
    );
    // It is checked on the bytes, before decoding: input that would be
    // refused for its encoding, as a DOCTYPE or as malformed is refused for
    // its size.
    let mut utf32 = vec![0x00, 0x00, 0xFE, 0xFF];
    utf32.extend("<r/>".chars().flat_map(|c| u32::from(c).to_be_bytes()));
    let doctype = b"<!DOCTYPE r><r/>".to_vec();
    let not_utf8 = b"<r>\xFF</r>".to_vec();
    for (what, bytes) in [
        ("a UTF-32 byte order mark", utf32),
        ("a DOCTYPE", doctype),
        ("invalid UTF-8", not_utf8),
    ] {
        let max = u64::try_from(bytes.len() - 1).unwrap_or(u64::MAX);
        assert_exceeded(
            what,
            &bytes,
            Limits::default().with_max_input_bytes(max),
            (Limit::InputBytes, max, None),
            "SACM-SEC-003/input-bytes",
        );
    }
}

// --- 3. The depth ----------------------------------------------------------------------

#[test]
fn lib_sec_003_the_depth_is_checked_before_parsing_at_the_first_element_too_deep() {
    let limits = Limits::default().with_max_depth(3);
    // The root is depth 1, the Claim 2 and its name 3.
    let claim = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><name content=\"c\"/>\
                 </argumentElement>";
    let three = package(claim);
    assert_reads_under("a depth-3 document", three.as_bytes(), limits);
    // In a wrapper, `xmi:XMI` is a level, so the name is depth 4.
    let four = format!(
        "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
         xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n\
         <sacm:ArgumentPackage xmi:id=\"p\">{claim}</sacm:ArgumentPackage>\n</xmi:XMI>"
    );
    assert_exceeded(
        "a depth-4 document",
        four.as_bytes(),
        limits,
        (Limit::Depth, 3, Some(position_of(&four, "<name"))),
        "SACM-SEC-003/depth",
    );
    // Comments, CDATA sections, processing instructions and `>` in a value
    // add no depth: the scan lets it through to the root check.
    let hidden = b"<a b=\">\"><!-- <x><x> --><![CDATA[<y>]]><?p <z>?></a>";
    for mode in MODES {
        let error = refusal(
            "markup that adds no depth",
            hidden,
            &under(Limits::default().with_max_depth(1), mode),
        );
        assert!(
            matches!(error, ReadError::InvalidRoot { .. }),
            "{ROW}: markup that adds no depth: a {mode:?} read is not InvalidRoot: {error:?}"
        );
    }
    // Unclosed input too deep is refused for its depth before it is parsed.
    assert_exceeded(
        "<a><b><c></a>",
        b"<a><b><c></a>",
        Limits::default().with_max_depth(2),
        (Limit::Depth, 2, Some(loc(1, 7))),
        "SACM-SEC-003/depth",
    );
    // The scan stops at a DOCTYPE, which is refused as before.
    for mode in MODES {
        let error = refusal(
            "a DOCTYPE document over the depth",
            b"<!DOCTYPE r><r><s><t/></s></r>",
            &under(Limits::default().with_max_depth(1), mode),
        );
        assert!(
            matches!(error, ReadError::Doctype { .. }),
            "{ROW}: a DOCTYPE over the depth: a {mode:?} read is not Doctype: {error:?}"
        );
    }
}

// --- 4. The node limit -----------------------------------------------------------------

#[test]
fn lib_sec_003_the_node_limit_is_the_parsers_and_has_no_location() {
    // The document node, the root and three comments: 5 nodes.
    let five = package("<!--1--><!--2--><!--3-->");
    let limits = Limits::default().with_max_nodes(5);
    assert_reads_under("a document of 5 nodes", five.as_bytes(), limits);
    // A processing instruction is one more.
    let six = package("<!--1--><!--2--><!--3--><?p?>");
    assert_exceeded(
        "a document of 6 nodes",
        six.as_bytes(),
        limits,
        (Limit::Nodes, 5, None),
        "SACM-SEC-003/nodes",
    );
    // `<r/>` is 2 nodes, and a text node is one more.
    for (input, nodes) in [(&b"<r/>"[..], 2), (b"<r>x</r>", 3)] {
        for mode in MODES {
            let error = refusal(
                "a document of its node limit",
                input,
                &under(Limits::default().with_max_nodes(nodes), mode),
            );
            assert!(
                matches!(error, ReadError::InvalidRoot { .. }),
                "{ROW}: {nodes} nodes under a limit of {nodes}: a {mode:?} read is not \
                 InvalidRoot: {error:?}"
            );
        }
        assert_exceeded(
            "a document of one node more than its limit",
            input,
            Limits::default().with_max_nodes(nodes - 1),
            (Limit::Nodes, nodes - 1, None),
            "SACM-SEC-003/nodes",
        );
    }
}

// --- 5. The bindings in scope ----------------------------------------------------------

#[test]
fn lib_sec_003_bindings_in_scope_count_one_per_prefix() {
    let two = Limits::default().with_max_namespace_bindings(2);
    // A root declaring `sacm` and `xmi`: 2, `xml` not counted.
    assert_reads_under("a root with sacm and xmi", package("").as_bytes(), two);
    // A child declaring `v` has 3 in scope.
    let child = package("<v:x xmlns:v=\"urn:v\"/>");
    assert_exceeded(
        "a child declaring v",
        child.as_bytes(),
        two,
        (
            Limit::NamespaceBindings,
            2,
            Some(position_of(&child, "<v:x")),
        ),
        "SACM-SEC-003/namespace-bindings",
    );
    // Redeclaring an in-scope prefix adds nothing.
    let redeclared = package(
        "<argumentElement xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
         xmi:type=\"sacm:Claim\" xmi:id=\"c\"/>",
    );
    assert_reads_under("a redeclared prefix", redeclared.as_bytes(), two);
    let rebound = package("<x xmlns:sacm=\"urn:other\"/>");
    assert_reads_under("a prefix bound to another URI", rebound.as_bytes(), two);
    // A sibling's bindings do not count: each child has 3 in scope.
    let siblings = package("<v:x xmlns:v=\"urn:v\"/><w:x xmlns:w=\"urn:w\"/>");
    assert_reads_under(
        "two siblings declaring one each",
        siblings.as_bytes(),
        Limits::default().with_max_namespace_bindings(3),
    );
    // `xmlns=""` counts.
    let undeclared = package("<x xmlns=\"\"/>");
    assert_exceeded(
        "xmlns=\"\" on a child",
        undeclared.as_bytes(),
        two,
        (
            Limit::NamespaceBindings,
            2,
            Some(position_of(&undeclared, "<x")),
        ),
        "SACM-SEC-003/namespace-bindings",
    );
    // `xml` does not, declared or not.
    let xml = package("<x xmlns:xml=\"http://www.w3.org/XML/1998/namespace\"/>");
    assert_reads_under("xmlns:xml on a child", xml.as_bytes(), two);
}

// --- 6. Namespace copies ---------------------------------------------------------------

#[test]
fn lib_sec_003_namespace_copies_count_against_the_node_limit() {
    // `r` declares one and has 1 in scope; `x` declares one and has 2: 3
    // copies in all.
    let input = "<r xmlns:a=\"u\"><x xmlns:b=\"v\"/></r>";
    assert_exceeded(
        "3 copies under a node limit of 2",
        input.as_bytes(),
        Limits::default().with_max_nodes(2),
        (Limit::Nodes, 2, Some(position_of(input, "<x"))),
        "SACM-SEC-003/namespace-copies",
    );
    // Under a limit of 3 the copies and the 3 nodes fit: the root check
    // refuses `r`.
    for mode in MODES {
        let error = refusal(
            "3 copies under a node limit of 3",
            input.as_bytes(),
            &under(Limits::default().with_max_nodes(3), mode),
        );
        assert!(
            matches!(error, ReadError::InvalidRoot { .. }),
            "{ROW}: 3 copies under a node limit of 3: a {mode:?} read is not InvalidRoot: \
             {error:?}"
        );
    }
}

// --- 7. Kept namespace text ------------------------------------------------------------

#[test]
fn lib_sec_003_kept_namespace_text_counts_against_the_size_limit() {
    let input = ten_preserved();
    let len = input.len();
    assert_eq!(len, 269, "{ROW}: setup: the input's size");
    // Under a limit of its own size the count passes it at the third child:
    // 3 x 95 = 285 > 269.
    let max = u64::try_from(len).unwrap_or(u64::MAX);
    let passing = len / KEPT_PER_CHILD;
    assert_exceeded(
        "ten preserved children under a limit of the input's size",
        input.as_bytes(),
        Limits::default().with_max_input_bytes(max),
        (
            Limit::InputBytes,
            max,
            Some(nth_position_of(&input, "<v:x", passing)),
        ),
        "SACM-SEC-003/kept-namespaces",
    );
    // The count is 950 in all: a limit of 950 reads it, and 949 passes it at
    // the tenth child.
    let total = 10 * KEPT_PER_CHILD;
    let total_max = u64::try_from(total).unwrap_or(u64::MAX);
    assert_reads_under(
        "ten preserved children under a limit of what they keep",
        input.as_bytes(),
        Limits::default().with_max_input_bytes(total_max),
    );
    assert_exceeded(
        "ten preserved children under one byte less than they keep",
        input.as_bytes(),
        Limits::default().with_max_input_bytes(total_max - 1),
        (
            Limit::InputBytes,
            total_max - 1,
            Some(nth_position_of(&input, "<v:x", 9)),
        ),
        "SACM-SEC-003/kept-namespaces",
    );
}

// --- 8. The parser's namespace capacity ------------------------------------------------

/// A root `r` with 65,536 children, each declaring a distinct `pN`.
fn many_prefixes() -> Vec<u8> {
    let mut input = String::from("<r>");
    for n in 0..65_536 {
        let _ = write!(input, "<x xmlns:p{n}=\"u\"/>");
    }
    input.push_str("</r>");
    input.into_bytes()
}

#[test]
fn lib_sec_003_65536_namespace_prefixes_are_sacm_sec_002_without_a_location() {
    let input = many_prefixes();
    let strict = refusal("65,536 prefixes", &input, &options(Mode::Strict));
    let tolerant = refusal("65,536 prefixes", &input, &options(Mode::Tolerant));
    assert!(
        tolerant == strict,
        "{ROW}: 65,536 prefixes: the modes refuse differently"
    );
    assert!(
        matches!(strict, ReadError::Unsupported { .. }),
        "{ROW}: 65,536 prefixes are not Unsupported: {strict:?}"
    );
    assert_eq!(
        (strict.code().as_str(), strict.template(), strict.location()),
        ("SACM-SEC-002", "SACM-SEC-002/namespaces", None),
        "{ROW}: 65,536 prefixes: the code, template and location"
    );
}

// --- 9. Both modes ---------------------------------------------------------------------

#[test]
fn lib_sec_003_a_breach_is_the_same_in_both_modes() {
    let preserved = ten_preserved();
    let ten_max = u64::try_from(preserved.len()).unwrap_or(u64::MAX);
    let cases: Vec<(&str, Vec<u8>, Limits)> = vec![
        (
            "the input's size",
            format!("{} ", package("")).into_bytes(),
            Limits::default().with_max_input_bytes(10),
        ),
        (
            "the depth",
            b"<a><b><c></a>".to_vec(),
            Limits::default().with_max_depth(2),
        ),
        (
            "the nodes",
            package("<!--1--><!--2-->").into_bytes(),
            Limits::default().with_max_nodes(3),
        ),
        (
            "the bindings in scope",
            package("<v:x xmlns:v=\"urn:v\"/>").into_bytes(),
            Limits::default().with_max_namespace_bindings(2),
        ),
        (
            "the namespace copies",
            b"<r xmlns:a=\"u\"><x xmlns:b=\"v\"/></r>".to_vec(),
            Limits::default().with_max_nodes(2),
        ),
        (
            "the kept namespace text",
            preserved.into_bytes(),
            Limits::default().with_max_input_bytes(ten_max),
        ),
    ];
    for (what, input, limits) in cases {
        let strict = refusal(what, &input, &under(limits, Mode::Strict));
        let tolerant = refusal(what, &input, &under(limits, Mode::Tolerant));
        assert!(
            matches!(strict, ReadError::LimitExceeded { .. }),
            "{ROW}: {what}: not LimitExceeded: {strict:?}"
        );
        assert_eq!(
            tolerant, strict,
            "{ROW}: {what}: a tolerant read refuses it otherwise than a strict one"
        );
    }
}

// --- 10. The default edges -------------------------------------------------------------

#[test]
fn lib_sec_003_the_default_edges() {
    // 32 MiB + 1 bytes of white space.
    let spaces = vec![b' '; DEFAULT_INPUT_BYTES + 1];
    assert_sec_003_at_the_defaults(
        "32 MiB + 1 bytes",
        &spaces,
        "SACM-SEC-003/input-bytes",
        None,
    );
    drop(spaces);
    // Depth 256 reads; 257 does not, at the 257th element.
    let deep = chain(DEFAULT_DEPTH - 1);
    for mode in MODES {
        read_ok("a depth-256 chain", deep.as_bytes(), &options(mode));
    }
    let deeper = chain(DEFAULT_DEPTH);
    assert_sec_003_at_the_defaults(
        "a depth-257 chain",
        deeper.as_bytes(),
        "SACM-SEC-003/depth",
        Some(nth_position_of(&deeper, "<v:x", DEFAULT_DEPTH - 1)),
    );
    // 256 bindings in scope read; 257 do not, at the root.
    let bindings = root_with_bindings(DEFAULT_BINDINGS - 2);
    for mode in MODES {
        read_ok("256 bindings in scope", bindings.as_bytes(), &options(mode));
    }
    let more = root_with_bindings(DEFAULT_BINDINGS - 1);
    assert_sec_003_at_the_defaults(
        "257 bindings in scope",
        more.as_bytes(),
        "SACM-SEC-003/namespace-bindings",
        Some(loc(1, 1)),
    );
}

// --- 12. The output bound of write -----------------------------------------------------

/// The bound of `write`'s docs, "# Output size": `(2·D + 48)·B + 6·M + 8192`.
fn bound(depth: usize, input: usize, kept: usize) -> usize {
    (2 * depth + 48) * input + 6 * kept + 8192
}

/// A chain of `AssuranceCasePackage`s `depth` deep in all: the root, nested
/// packages down to depth `depth - 3`, then an ArgumentPackage, Claims and
/// their names.
fn package_chain(depth: usize) -> String {
    let nested = depth - 4;
    let mut input = format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"a0\">");
    for n in 1..=nested {
        let _ = write!(
            input,
            "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackage\" xmi:id=\"a{n}\">"
        );
    }
    input.push_str("<argumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap\">");
    for n in 0..10 {
        let _ = write!(
            input,
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c{n}\"><name content=\"c{n}\"/>\
             </argumentElement>"
        );
    }
    input.push_str("</argumentPackage>");
    input.push_str(&"</assuranceCasePackage>".repeat(nested));
    input.push_str("</sacm:AssuranceCasePackage>");
    input
}

/// 200 `TaggedValue`s of an `ArgumentPackage` in the C++ shape `key=""`, which
/// a tolerant read reads as the standard form.
fn tagged_values() -> String {
    let mut body = String::from("<name content=\"P\"/>");
    for n in 0..200 {
        let _ = write!(body, "<taggedValue xmi:id=\"t{n}\" key=\"\"/>");
    }
    package(&body)
}

/// Claims whose names are full of `&` and `"`, in CDATA sections of the
/// element form and in single-quoted values.
fn escaped_names() -> String {
    let mut body = String::new();
    for n in 0..100 {
        let _ = write!(
            body,
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c{n}\">\
             <name><content><![CDATA[&\"&\"&\"&\"&\"&\"]]></content></name></argumentElement>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"d{n}\">\
             <name content='\"\"\"\"&amp;&amp;'/></argumentElement>"
        );
    }
    package(&body)
}

/// 1,000 preserved `<v0:x/>` under 200 bindings in scope, and the namespace
/// text they keep: each keeps the 198 bindings that are not `sacm` and `xmi`
/// (`prefix + URI + 10` each) and its name's URI.
fn preserved_under_bindings() -> (String, usize) {
    let mut start = format!("<sacm:ArgumentPackage{NS}");
    let mut per_child = 0;
    for n in 0..198 {
        let prefix = format!("v{n}");
        let uri = format!("urn:v{n}");
        per_child += prefix.len() + uri.len() + 10;
        let _ = write!(start, " xmlns:{prefix}=\"{uri}\"");
    }
    per_child += "urn:v0".len();
    start.push_str(" xmi:id=\"p\">");
    let input = format!("{start}{}{ROOT_END}", "<v0:x/>".repeat(1000));
    (input, 1000 * per_child)
}

#[test]
fn lib_sec_003_write_stays_within_the_documented_bound() {
    const D: usize = 64;
    let (preserved, kept) = preserved_under_bindings();
    let inputs = [
        (
            "a chain of AssuranceCasePackages 64 deep",
            package_chain(D),
            0,
        ),
        ("TaggedValues in the C++ key=\"\" shape", tagged_values(), 0),
        ("names full of & and \"", escaped_names(), 0),
        (
            "1,000 preserved elements under 200 bindings",
            preserved,
            kept,
        ),
    ];
    for (what, input, kept) in inputs {
        let b = input.len();
        let m = b.max(kept);
        let limits = Limits::default()
            .with_max_depth(u64::try_from(D).unwrap_or(u64::MAX))
            .with_max_input_bytes(u64::try_from(m).unwrap_or(u64::MAX));
        let document =
            read_ok(what, input.as_bytes(), &under(limits, Mode::Tolerant)).into_document();
        let limit = bound(D, b, m);
        let mut wrote = 0;
        for mode in MODES {
            // A strict write may refuse (preserved content); the bound is for
            // what is written.
            if let Ok(written) = xmi::write(&document, &WriteOptions::default().with_mode(mode)) {
                wrote += 1;
                assert!(
                    written.len() <= limit,
                    "{ROW}: {what}: a {mode:?} write of {b} input bytes under depth {D} and \
                     size {m} writes {} bytes, more than the bound {limit}",
                    written.len()
                );
            }
        }
        assert!(wrote > 0, "{ROW}: {what}: no write mode writes it");
    }
}
