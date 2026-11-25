//! The attribute limit (LIB-SEC-007; ADR 0041 amendment 2, D8 to D10).
//!
//! roxmltree 0.21.2 compares each attribute of an element with the
//! element's earlier ones, so its time per element grows with the square of
//! that element's attributes. `Limits::with_max_attributes` bounds the
//! attributes of one element that `xmi::read` accepts, in both modes, by
//! default 256: every attribute in a start tag counts, `xml:` attributes and
//! repeats included, the first namespace declaration of each prefix not
//! counted. LIB-SEC-003's scan before parsing counts them at each start tag,
//! after the depth, the bindings in scope and the namespace copies, and
//! refuses an element over the limit with `ReadError::LimitExceeded` (SACM-SEC-003,
//! `Limit::Attributes`, template `SACM-SEC-003/attributes`) at the `<` of its
//! start tag, wherever it stands. A read at the defaults is then linear in
//! its input.

use std::fmt::Write as _;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Diagnostic, Severity, SourceLocation};
use sacm::model::ElementKind;
use sacm::xmi::{self, Limit, Limits, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "LIB-SEC-007";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The start tag of a root ArgumentPackage `p`, open for more attributes: it
/// declares `sacm`, `xmi` and `v` (not counted) and has one counted
/// attribute, `xmi:id`.
const R: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:v=\"urn:v\"",
    " xmi:id=\"p\"",
);

/// The end tag of [`R`].
const END: &str = "</sacm:ArgumentPackage>";

/// The default attribute limit.
const DEFAULT_ATTRIBUTES: u64 = 256;

/// How long one read of 100,000 attributes on one element may take: the
/// scan refuses the element at its `<` well under that, and the parser's
/// duplicate-attribute check over them takes minutes (about 214 s in a
/// debug build; the budget of `limits.rs`).
const DEADLINE: Duration = Duration::from_secs(5);

/// How long one read at the default limit may take: the budget of the
/// linearity tests (`emf_dialect.rs`), which holds a debug read of a few
/// hundred thousand attributes on a slow runner.
const LINEAR_DEADLINE: Duration = Duration::from_secs(30);

// --- Helpers ---------------------------------------------------------------------------

fn options(mode: Mode) -> ReadOptions {
    ReadOptions::default().with_mode(mode)
}

/// `options(mode)` under `limits`.
fn under(limits: Limits, mode: Mode) -> ReadOptions {
    options(mode).with_limits(limits)
}

/// The default limits but `max_attributes`, which is `max`.
fn attributes(max: u64) -> Limits {
    Limits::default().with_max_attributes(max)
}

fn loc(line: u32, column: u32) -> SourceLocation {
    SourceLocation::new(line, column)
        .unwrap_or_else(|| panic!("{ROW}: setup: ({line}, {column}) is no location"))
}

/// The location of the only `needle` in `text`, whose lines end with LF:
/// the line, and the column in characters.
fn position_of(text: &str, needle: &str) -> SourceLocation {
    let mut found = text.match_indices(needle).map(|(offset, _)| offset);
    let (Some(offset), None) = (found.next(), found.next()) else {
        panic!("{ROW}: setup: {needle:?} is not in the input exactly once");
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
            "{ROW}: {what}: the read returns a document, with {} diagnostics",
            output.diagnostics().len()
        ),
        Err(error) => error,
    }
}

/// The refusal of `input` read under `limits` in both modes, which must be
/// the same.
fn refused_alike(what: &str, input: &[u8], limits: Limits) -> ReadError {
    let strict = refusal(what, input, &under(limits, Mode::Strict));
    let tolerant = refusal(what, input, &under(limits, Mode::Tolerant));
    assert_eq!(
        tolerant, strict,
        "{ROW}: {what}: the modes refuse differently"
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

/// Asserts that `input`, read under `limits` in both modes alike, is
/// `LimitExceeded` for `limit` with `max` at `location`, with the code
/// SACM-SEC-003 and `template`, and returns the refusal.
fn assert_limit(
    what: &str,
    input: &[u8],
    limits: Limits,
    expected: (Limit, u64, Option<SourceLocation>),
    template: &str,
) -> ReadError {
    let error = refused_alike(what, input, limits);
    let ReadError::LimitExceeded {
        limit,
        max,
        location,
        ..
    } = &error
    else {
        panic!("{ROW}: {what}: not LimitExceeded: {error:?} ({error})");
    };
    assert_eq!(
        (*limit, *max, *location),
        expected,
        "{ROW}: {what}: the limit, its value and the location ({error})"
    );
    assert_eq!(
        (error.code().as_str(), error.template(), error.location()),
        ("SACM-SEC-003", template, expected.2),
        "{ROW}: {what}: code(), template() and location()"
    );
    error
}

/// Asserts that `input`, read under `limits` in both modes alike, is
/// refused for its attributes: `LimitExceeded { limit: Attributes, max,
/// location: Some(at) }`, SACM-SEC-003, `SACM-SEC-003/attributes`, the
/// argument `max` and its Display.
fn assert_exceeded(what: &str, input: &[u8], limits: Limits, max: u64, at: SourceLocation) {
    let error = assert_limit(
        what,
        input,
        limits,
        (Limit::Attributes, max, Some(at)),
        "SACM-SEC-003/attributes",
    );
    assert_eq!(
        counts_of(&error),
        [("max", Some(max))],
        "{ROW}: {what}: the argument max"
    );
    assert_eq!(
        error.to_string(),
        format!(
            "an element has more attributes than the limit of {max} at line {}, column {}",
            at.line(),
            at.column()
        ),
        "{ROW}: {what}: the Display of the refusal"
    );
}

/// Asserts that `input`, read under `limits`, reads in both modes, and
/// returns the outputs, strict first.
fn assert_reads_under(what: &str, input: &[u8], limits: Limits) -> [ReadOutput; 2] {
    MODES.map(|mode| read_ok(&format!("{what} ({mode:?})"), input, &under(limits, mode)))
}

/// The severity of a SACM-XMI-003 diagnostic in `mode`: an Error strict, a
/// Warning tolerant.
fn kept_severity(mode: Mode) -> Severity {
    match mode {
        Mode::Strict => Severity::Error,
        _ => Severity::Warning,
    }
}

/// A diagnostic as `(code, template, severity, affected ids)`.
type Summary = (String, &'static str, Severity, Vec<String>);

fn summary(diagnostic: &Diagnostic) -> Summary {
    (
        diagnostic.code().as_str().to_owned(),
        diagnostic.template(),
        diagnostic.severity(),
        diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    )
}

fn summaries(output: &ReadOutput) -> Vec<Summary> {
    output.diagnostics().iter().map(summary).collect()
}

/// `count` SACM-XMI-003 diagnostics of `template` in `mode`, each affecting
/// `holder`.
fn kept(template: &'static str, mode: Mode, holder: &str, count: usize) -> Vec<Summary> {
    vec![
        (
            "SACM-XMI-003".to_owned(),
            template,
            kept_severity(mode),
            vec![holder.to_owned()],
        );
        count
    ]
}

/// The read of `input` in `mode` under the default limits on a worker
/// thread, which must return within `deadline`.
fn read_in_time(
    what: &str,
    input: String,
    mode: Mode,
    deadline: Duration,
) -> Result<ReadOutput, ReadError> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(input.as_bytes(), &options(mode)));
    });
    match receiver.recv_timeout(deadline) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            panic!("{ROW}: {what}: a {mode:?} read did not return within {deadline:?}")
        }
        Err(RecvTimeoutError::Disconnected) => panic!("{ROW}: {what}: a {mode:?} read panicked"),
    }
}

/// `count` attributes ` {prefix}aK="value(K)"`, K from 0.
fn attribute_list(prefix: &str, count: usize, value: impl Fn(usize) -> String) -> String {
    let mut list = String::new();
    for k in 0..count {
        let _ = write!(list, " {prefix}a{k}=\"{}\"", value(k));
    }
    list
}

/// A Claim `c1` with `xmi:type`, `xmi:id` and `count` kept attributes
/// `v:aK="K"`, in a root of one counted attribute.
fn claim_with(count: usize) -> String {
    format!(
        "{R}><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{}/>{END}",
        attribute_list("v:", count, |k| k.to_string())
    )
}

/// The kind of `c1` in `output`.
fn kind_of_c1(what: &str, output: &ReadOutput) -> ElementKind {
    output
        .document()
        .get("c1")
        .unwrap_or_else(|| panic!("{ROW}: {what}: the document has no element c1"))
        .kind()
}

// --- 1. The limit, its default and its setter ------------------------------------------

#[test]
fn lib_sec_007_the_attribute_limit_has_its_default_and_setter() {
    let defaults = Limits::default();
    assert_eq!(
        defaults.max_attributes(),
        DEFAULT_ATTRIBUTES,
        "{ROW}: the default attribute limit"
    );
    let others = |limits: Limits| {
        (
            limits.max_input_bytes(),
            limits.max_depth(),
            limits.max_nodes(),
            limits.max_namespace_bindings(),
        )
    };
    let seven = defaults.with_max_attributes(7);
    assert_eq!(
        (seven.max_attributes(), others(seven)),
        (7, (33_554_432, 256, 1_000_000, 256)),
        "{ROW}: with_max_attributes sets only the attribute limit"
    );
    assert_eq!(
        [
            defaults.with_max_input_bytes(7).max_attributes(),
            defaults.with_max_depth(7).max_attributes(),
            defaults.with_max_nodes(7).max_attributes(),
            defaults.with_max_namespace_bindings(7).max_attributes(),
        ],
        [DEFAULT_ATTRIBUTES; 4],
        "{ROW}: each other with_max_* leaves the attribute limit"
    );
    assert_eq!(
        ReadOptions::default()
            .with_limits(Limits::default().with_max_attributes(9))
            .limits()
            .max_attributes(),
        9,
        "{ROW}: with_limits round-trips the attribute limit"
    );
    assert_eq!(
        Limit::Attributes.to_string(),
        "attributes",
        "{ROW}: the Display of Limit::Attributes"
    );
}

// --- 2. Where an element stands --------------------------------------------------------

#[test]
fn lib_sec_007_an_element_over_the_attribute_limit_is_refused_at_its_start_tag() {
    let three = attributes(3);
    // A root: xmi:id, v:a and v:b are 3, the declarations do not count.
    let root = format!("{R} v:a=\"1\" v:b=\"2\"/>");
    assert_reads_under("a root of 3 attributes", root.as_bytes(), three);
    let root = format!("{R} v:a=\"1\" v:b=\"2\" v:c=\"3\"/>");
    assert_exceeded(
        "a root of 4 attributes",
        root.as_bytes(),
        three,
        3,
        loc(1, 1),
    );
    let display = refused_alike("a root of 4 attributes", root.as_bytes(), three).to_string();
    assert_eq!(
        display, "an element has more attributes than the limit of 3 at line 1, column 1",
        "{ROW}: a root of 4 attributes: the Display"
    );
    // A SACM element: xmi:type, xmi:id, v:a and v:b are 4.
    let claim = format!(
        "{R}><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" v:a=\"1\" v:b=\"2\"/>{END}"
    );
    assert_exceeded(
        "a Claim of 4 attributes",
        claim.as_bytes(),
        three,
        3,
        position_of(&claim, "<argumentElement"),
    );
    for output in assert_reads_under("a Claim of 4 attributes", claim.as_bytes(), attributes(4)) {
        assert_eq!(
            kind_of_c1("a Claim of 4 attributes", &output),
            ElementKind::Claim,
            "{ROW}: a Claim of 4 attributes under a limit of 4: c1 is not a Claim"
        );
    }
    // A foreign element, kept whole.
    let foreign = format!("{R}><v:x a=\"1\" b=\"2\" c=\"3\" d=\"4\"/>{END}");
    assert_exceeded(
        "a foreign element of 4 attributes",
        foreign.as_bytes(),
        three,
        3,
        position_of(&foreign, "<v:x"),
    );
    let outputs = assert_reads_under(
        "a foreign element of 4 attributes",
        foreign.as_bytes(),
        attributes(4),
    );
    for (mode, output) in MODES.into_iter().zip(&outputs) {
        assert_eq!(
            summaries(output),
            kept("SACM-XMI-003/element-kept", mode, "p", 1),
            "{ROW}: a foreign element of 4 attributes under a limit of 4: a {mode:?} read \
             does not keep it with one SACM-XMI-003"
        );
    }
    // The XMI element.
    let wrapper = "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                   a=\"1\" b=\"2\" c=\"3\" d=\"4\"/>";
    assert_exceeded(
        "an xmi:XMI of 4 attributes",
        wrapper.as_bytes(),
        three,
        3,
        loc(1, 1),
    );
    // Under a limit of 4 it passes the scan, and holds no package.
    let error = refused_alike(
        "an xmi:XMI of 4 attributes under a limit of 4",
        wrapper.as_bytes(),
        attributes(4),
    );
    assert!(
        matches!(error, ReadError::NoPackage { .. }),
        "{ROW}: an xmi:XMI of 4 attributes under a limit of 4 is not NoPackage: {error:?}"
    );
}

// --- 3. What counts --------------------------------------------------------------------

#[test]
fn lib_sec_007_namespace_declarations_do_not_count_and_every_other_attribute_does() {
    let two = attributes(2);
    // xmi:id and v:a are 2; xmlns, xmlns:xml and the three xmlns:p are not
    // counted.
    let declarations = format!(
        "{R} xmlns=\"urn:d\" xmlns:xml=\"http://www.w3.org/XML/1998/namespace\" v:a=\"1\"/>"
    );
    for mode in MODES {
        if let Err(error) = xmi::read(declarations.as_bytes(), &under(two, mode)) {
            assert!(
                !matches!(error, ReadError::LimitExceeded { .. }),
                "{ROW}: namespace declarations: a {mode:?} read under a limit of 2 counts \
                 them: {error:?}"
            );
        }
    }
    // An xml: attribute counts: xmi:id, xml:lang and v:a are 3.
    let xml = format!("{R} xml:lang=\"en\" v:a=\"1\"/>");
    assert_exceeded("an xml:lang attribute", xml.as_bytes(), two, 2, loc(1, 1));
    // A repeated attribute counts each time, and the limit comes before the
    // parser's duplicate check.
    let repeated = b"<a b=\"1\" b=\"2\" c=\"3\"/>";
    assert_exceeded("a repeated attribute", repeated, two, 2, loc(1, 1));
    let error = refused_alike("a repeated attribute", repeated, attributes(3));
    assert!(
        matches!(error, ReadError::Malformed { .. }),
        "{ROW}: a repeated attribute under a limit of 3 is not Malformed: {error:?}"
    );
    assert_eq!(
        (error.code().as_str(), error.template()),
        ("SACM-XML-001", "SACM-XML-001/duplicate-attribute"),
        "{ROW}: a repeated attribute under a limit of 3: the code and template of {error}"
    );
}

// --- 4. The check order ----------------------------------------------------------------

#[test]
fn lib_sec_007_the_limit_is_checked_before_parsing_after_the_other_scan_limits() {
    let one = attributes(1);
    // The depth comes first at the same start tag.
    let deep = b"<a><b c=\"1\" d=\"2\"/></a>";
    assert_limit(
        "a depth-2 element of 2 attributes under depth 1",
        deep,
        one.with_max_depth(1),
        (Limit::Depth, 1, Some(loc(1, 4))),
        "SACM-SEC-003/depth",
    );
    assert_exceeded(
        "a depth-2 element of 2 attributes under depth 2",
        deep,
        one.with_max_depth(2),
        1,
        loc(1, 4),
    );
    // Then the bindings in scope.
    let declaring = b"<a><b xmlns:p=\"u\" c=\"1\" d=\"2\"/></a>";
    assert_limit(
        "an element of 1 binding and 2 attributes under 0 bindings",
        declaring,
        one.with_max_namespace_bindings(0),
        (Limit::NamespaceBindings, 0, Some(loc(1, 4))),
        "SACM-SEC-003/namespace-bindings",
    );
    assert_exceeded(
        "an element of 1 binding and 2 attributes under 1 binding",
        declaring,
        one.with_max_namespace_bindings(1),
        1,
        loc(1, 4),
    );
    // Then the namespace copies: `a` has 1 in scope, `b` 2, 3 in all.
    let copies = b"<a xmlns:p=\"u\"><b xmlns:q=\"v\" c=\"1\" d=\"2\"/></a>";
    assert_limit(
        "3 copies and an element of 2 attributes under 2 nodes",
        copies,
        one.with_max_nodes(2),
        (Limit::Nodes, 2, Some(loc(1, 16))),
        "SACM-SEC-003/namespace-copies",
    );
    assert_exceeded(
        "3 copies and an element of 2 attributes under 3 nodes",
        copies,
        one.with_max_nodes(3),
        1,
        loc(1, 16),
    );
    // Before parsing: malformed input after the element is not reached.
    assert_exceeded(
        "an element of 2 attributes before malformed input",
        b"<a b=\"1\" c=\"2\"><x></a>",
        one,
        1,
        loc(1, 1),
    );
    // A DOCTYPE still stops the scan.
    for mode in MODES {
        let error = refusal(
            "a DOCTYPE before an element of 2 attributes",
            b"<!DOCTYPE a><a b=\"1\" c=\"2\"/>",
            &under(one, mode),
        );
        assert!(
            matches!(error, ReadError::Doctype { .. }),
            "{ROW}: a DOCTYPE before an element of 2 attributes: a {mode:?} read is not \
             Doctype: {error:?}"
        );
    }
}

// --- 5. The default edge ---------------------------------------------------------------

#[test]
fn lib_sec_007_the_default_edge() {
    // xmi:id and 255 kept v:aK are 256.
    let at_limit = format!("{R}{}/>", attribute_list("v:", 255, |_| "1".to_owned()));
    let outputs = assert_reads_under(
        "a root of 256 attributes",
        at_limit.as_bytes(),
        Limits::default(),
    );
    for (mode, output) in MODES.into_iter().zip(&outputs) {
        assert_eq!(
            summaries(output),
            kept("SACM-XMI-003/attribute-kept", mode, "p", 255),
            "{ROW}: a root of 256 attributes: a {mode:?} read does not keep the 255 with \
             SACM-XMI-003"
        );
    }
    let over = format!("{R}{}/>", attribute_list("v:", 256, |_| "1".to_owned()));
    assert_exceeded(
        "a root of 257 attributes",
        over.as_bytes(),
        Limits::default(),
        DEFAULT_ATTRIBUTES,
        loc(1, 1),
    );
}

// --- 6. A hostile element --------------------------------------------------------------

/// The refusal of `input`, read in each mode at the defaults within
/// [`DEADLINE`], which must be the same in both.
fn refused_in_time(what: &str, input: &str) -> ReadError {
    let mut refusals = Vec::new();
    for mode in MODES {
        match read_in_time(what, input.to_owned(), mode, DEADLINE) {
            Ok(output) => panic!(
                "{ROW}: {what}: a {mode:?} read returns a document, with {} diagnostics",
                output.diagnostics().len()
            ),
            Err(error) => refusals.push(error),
        }
    }
    let [strict, tolerant] = &refusals[..] else {
        panic!("{ROW}: {what}: not one refusal per mode");
    };
    assert_eq!(
        tolerant, strict,
        "{ROW}: {what}: the modes refuse differently"
    );
    strict.clone()
}

/// Asserts that `input`, read in each mode at the defaults within
/// [`DEADLINE`], is refused for its attributes at `at`.
fn assert_refused_in_time(what: &str, input: &str, at: SourceLocation) {
    let strict = &refused_in_time(what, input);
    assert!(
        matches!(
            strict,
            ReadError::LimitExceeded {
                limit: Limit::Attributes,
                max: DEFAULT_ATTRIBUTES,
                location: Some(location),
                ..
            } if *location == at
        ),
        "{ROW}: {what}: not LimitExceeded {{ Attributes, 256 }} at {at:?}: {strict:?}"
    );
    assert_eq!(
        (strict.code().as_str(), strict.template()),
        ("SACM-SEC-003", "SACM-SEC-003/attributes"),
        "{ROW}: {what}: the code and template"
    );
}

#[test]
fn lib_sec_007_100000_attributes_on_one_element_are_refused_in_time() {
    let what = "a Claim of 100,000 kept attributes";
    let claim = format!(
        "{R}><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{}>\
         <name content=\"c\"/></argumentElement>{END}",
        attribute_list("v:", 100_000, |k| k.to_string())
    );
    assert_refused_in_time(what, &claim, position_of(&claim, "<argumentElement"));
    drop(claim);
    let what = "a foreign element of 100,000 attributes";
    let foreign = format!(
        "{R}><v:x{}/>{END}",
        attribute_list("", 100_000, |k| k.to_string())
    );
    assert_refused_in_time(what, &foreign, position_of(&foreign, "<v:x"));
}

// --- 7. Linear at the default ----------------------------------------------------------

/// Reads `input` in each mode within [`LINEAR_DEADLINE`], asserts that each
/// gives `count` SACM-XMI-003 diagnostics of `template` affecting `holder`
/// (`holders(n)` for the `n`th), writes the tolerant read tolerantly,
/// asserts that the output holds `marker` exactly `markers` times, and that
/// a tolerant read of it is `==` to the tolerant read, with no semantic
/// difference.
fn assert_linear_round_trip(
    what: &str,
    input: &str,
    expected: (&'static str, usize, &dyn Fn(usize) -> String),
    marker: (&str, usize),
) {
    let (template, count, holders) = expected;
    let mut tolerant = None;
    for mode in MODES {
        let output = match read_in_time(what, input.to_owned(), mode, LINEAR_DEADLINE) {
            Ok(output) => output,
            Err(error) => panic!("{ROW}: {what}: a {mode:?} read refuses it: {error:?} ({error})"),
        };
        let found = summaries(&output);
        assert_eq!(
            found.len(),
            count,
            "{ROW}: {what}: a {mode:?} read gives {} diagnostics, not {count}",
            found.len()
        );
        for (n, diagnostic) in found.iter().enumerate() {
            assert_eq!(
                diagnostic,
                &kept(template, mode, &holders(n), 1)[0],
                "{ROW}: {what}: a {mode:?} read: diagnostic {n}"
            );
        }
        tolerant = Some(output);
    }
    let Some(tolerant) = tolerant else {
        panic!("{ROW}: {what}: no tolerant read");
    };
    let written = match xmi::write(
        tolerant.document(),
        &WriteOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(written) => written,
        Err(error) => panic!("{ROW}: {what}: a tolerant write refuses it: {error:?} ({error})"),
    };
    let (text, times) = marker;
    assert_eq!(
        written.matches(text).count(),
        times,
        "{ROW}: {what}: the tolerant write does not hold {text:?} {times} times"
    );
    let back = match read_in_time(what, written, Mode::Tolerant, LINEAR_DEADLINE) {
        Ok(back) => back,
        Err(error) => panic!(
            "{ROW}: {what}: a tolerant read of the tolerant write refuses it: {error:?} ({error})"
        ),
    };
    assert!(
        back.document() == tolerant.document(),
        "{ROW}: {what}: the tolerant write does not read back =="
    );
    assert_eq!(
        semantic_compare(tolerant.document(), back.document()),
        [],
        "{ROW}: {what}: the tolerant write reads back with semantic differences"
    );
}

#[test]
fn lib_sec_007_a_read_at_the_default_limit_is_linear_in_the_attributes() {
    // A: 1,000 foreign elements of 256 attributes each, kept whole.
    let mut a = format!("{R}><name content=\"p\"/>");
    for _ in 0..1_000 {
        let _ = write!(a, "<v:x{}/>", attribute_list("", 256, |_| "1".to_owned()));
    }
    a.push_str(END);
    assert_linear_round_trip(
        "1,000 foreign elements of 256 attributes",
        &a,
        ("SACM-XMI-003/element-kept", 1_000, &|_| "p".to_owned()),
        (" a255=\"1\"", 1_000),
    );
    drop(a);
    // B: 100 Claims of xmi:type, xmi:id and 254 kept attributes: 256 each.
    let mut b = format!("{R}><name content=\"p\"/>");
    for n in 0..100 {
        let _ = write!(
            b,
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c{n}\"{}>\
             <name content=\"c\"/></argumentElement>",
            attribute_list("v:", 254, |_| "1".to_owned())
        );
    }
    b.push_str(END);
    assert_linear_round_trip(
        "100 Claims of 254 kept attributes",
        &b,
        ("SACM-XMI-003/attribute-kept", 25_400, &|n| {
            format!("c{}", n / 254)
        }),
        (" v:a253=\"1\"", 100),
    );
}

// --- 8. A raised limit -----------------------------------------------------------------

#[test]
fn lib_sec_007_a_raised_limit_reads_and_keeps_them() {
    // xmi:type, xmi:id and 2,000 kept attributes: 2,002.
    let input = claim_with(2_000);
    let at = position_of(&input, "<argumentElement");
    assert_exceeded(
        "a Claim of 2,002 attributes at the defaults",
        input.as_bytes(),
        Limits::default(),
        DEFAULT_ATTRIBUTES,
        at,
    );
    let outputs = assert_reads_under(
        "a Claim of 2,002 attributes under a limit of 2,002",
        input.as_bytes(),
        attributes(2_002),
    );
    for (mode, output) in MODES.into_iter().zip(&outputs) {
        assert_eq!(
            summaries(output),
            kept("SACM-XMI-003/attribute-kept", mode, "c1", 2_000),
            "{ROW}: a Claim of 2,002 attributes under a limit of 2,002: a {mode:?} read does \
             not keep the 2,000 with SACM-XMI-003"
        );
        assert_eq!(
            kind_of_c1("a Claim of 2,002 attributes", output),
            ElementKind::Claim,
            "{ROW}: a Claim of 2,002 attributes under a limit of 2,002: c1 is not a Claim"
        );
    }
    assert_exceeded(
        "a Claim of 2,002 attributes under a limit of 2,001",
        input.as_bytes(),
        attributes(2_001),
        2_001,
        at,
    );
}

// --- 9. Serde --------------------------------------------------------------------------

#[cfg(feature = "serde")]
#[test]
fn lib_sec_007_the_refusal_serializes_with_its_limit() {
    let error = refusal(
        "<a b=\"1\" c=\"2\"/> under a limit of 1",
        b"<a b=\"1\" c=\"2\"/>",
        &under(attributes(1), Mode::Strict),
    );
    let expected = serde_json::json!({
        "type": "LimitExceeded",
        "code": "SACM-SEC-003",
        "template": "SACM-SEC-003/attributes",
        "arguments": {"max": 1},
        "limit": "attributes",
        "max": 1,
        "location": {"line": 1, "column": 1}
    });
    let value = serde_json::to_value(&error)
        .unwrap_or_else(|failure| panic!("{ROW}: the refusal does not serialize: {failure}"));
    assert_eq!(value, expected, "{ROW}: the JSON of the refusal");
    let back: ReadError = serde_json::from_value(value)
        .unwrap_or_else(|failure| panic!("{ROW}: the JSON does not read back: {failure}"));
    assert_eq!(back, error, "{ROW}: the JSON reads back otherwise");
    assert_eq!(
        serde_json::to_value(Limit::Attributes)
            .unwrap_or_else(|failure| panic!("{ROW}: Limit::Attributes: {failure}")),
        serde_json::json!("attributes"),
        "{ROW}: the JSON of Limit::Attributes"
    );
}

// --- 10. Repeated namespace declarations -----------------------------------------------

/// The `xml` namespace, which `xmlns:xml` may declare.
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// `count` copies of `item`.
fn repeated(item: &str, count: usize) -> String {
    item.repeat(count)
}

#[test]
fn lib_sec_007_a_repeated_namespace_declaration_counts() {
    let one = attributes(1);
    // K1: the second `xmlns` and `b` are 2 under a limit of 1.
    assert_exceeded(
        "K1: xmlns twice and one attribute",
        b"<a xmlns=\"\" xmlns=\"\" b=\"1\"/>",
        one,
        1,
        loc(1, 1),
    );
    // K2: the second `xmlns:xml` and `b` are 2.
    let xml = format!("<a xmlns:xml=\"{XML_NAMESPACE}\" xmlns:xml=\"{XML_NAMESPACE}\" b=\"1\"/>");
    assert_exceeded(
        "K2: xmlns:xml twice and one attribute",
        xml.as_bytes(),
        one,
        1,
        loc(1, 1),
    );
    // K3: the second `xmlns:p` and `b` are 2: the limit comes before the
    // parser's duplicate check, so it is LimitExceeded, not Malformed.
    assert_exceeded(
        "K3: xmlns:p twice and one attribute",
        b"<a xmlns:p=\"u\" xmlns:p=\"u\" b=\"1\"/>",
        one,
        1,
        loc(1, 1),
    );
    // K4: 100,000 `xmlns` and 100,000 `xmlns:xml` on a root, at the defaults.
    let what = "K4: a root of 100,000 xmlns and 100,000 xmlns:xml";
    let root = format!(
        "{R}{}{}/>",
        repeated(" xmlns=\"u\"", 100_000),
        repeated(&format!(" xmlns:xml=\"{XML_NAMESPACE}\""), 100_000)
    );
    assert_refused_in_time(what, &root, loc(1, 1));
    drop(root);
    // K5: 40,000 `xmlns=""` before the root's `sacm` and `xmi` declarations,
    // over 40,000 Claims, each of whose names the parser looks up past them.
    let what = "K5: a root of 40,000 xmlns over 40,000 Claims";
    let mut claims = String::new();
    for k in 0..40_000 {
        let _ = write!(
            claims,
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c{k}\"/>"
        );
    }
    let root = format!(
        "<sacm:ArgumentPackage{}\
         xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
         xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">{claims}{END}",
        repeated(" xmlns=\"\"", 40_000) + " "
    );
    assert_refused_in_time(what, &root, loc(1, 1));
    drop(root);
    // K6: 100,000 distinct prefixes on one start tag are over the bindings
    // limit, found in time: the check for a repeat is not a search of the
    // tag's earlier declarations.
    let what = "K6: a start tag of 100,000 distinct prefixes";
    let mut tag = String::from("<a");
    for k in 0..100_000 {
        let _ = write!(tag, " xmlns:p{k}=\"u\"");
    }
    tag.push_str("/>");
    let error = refused_in_time(what, &tag);
    assert!(
        matches!(
            error,
            ReadError::LimitExceeded {
                limit: Limit::NamespaceBindings,
                max: 256,
                location: Some(location),
                ..
            } if location == loc(1, 1)
        ),
        "{ROW}: {what}: not LimitExceeded {{ NamespaceBindings, 256 }} at (1, 1): {error:?}"
    );
    assert_eq!(
        (error.code().as_str(), error.template()),
        ("SACM-SEC-003", "SACM-SEC-003/namespace-bindings"),
        "{ROW}: {what}: the code and template"
    );
}
