//! The read limits (LIB-SEC-003; ADR 0041 D1, D2 and amendment 1):
//! [`Limits`], [`Limit`], the pre-parse scan that checks the nesting depth,
//! the namespace bindings in scope and the namespace copies before roxmltree
//! parses, and the [`Budget`] of what a read keeps besides the parsed tree.
//!
//! The scan is one linear pass in document order. It skips comments, CDATA
//! sections, processing instructions (the XML declaration included), quoted
//! attribute values and character data. At each start tag it checks, in this
//! order, the depth, the distinct bindings in scope, and the running total of
//! bindings in scope at the elements that declare one, which roxmltree 0.21
//! copies into its namespace table and which count against the node limit. A
//! self-closing tag pops at once. It stops without refusing at `<!` other
//! than a comment or a CDATA section (a DOCTYPE, which the parser refuses),
//! at a construct that is not closed, and at an end tag with nothing open:
//! the parser refuses what is left.

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;

use super::ReadError;
use super::input::{is_xml_space, locate};
use crate::diagnostic::template::{self, Template};
use crate::diagnostic::{Argument, Details, Diagnostic, SourceLocation};

/// Bounds on what [`read`](super::read) accepts, so that a hostile or broken
/// file is refused with [`ReadError::LimitExceeded`] (SACM-SEC-003) instead
/// of exhausting memory (LIB-SEC-003). Both modes apply them.
///
/// | Limit | Default | Checked |
/// |---|---|---|
/// | [`max_input_bytes`](Self::max_input_bytes) | 33,554,432 (32 MiB) | on the input's bytes, first; and, in one budget, on what the read keeps besides the parsed tree: the namespace text preserved content keeps, as each element or attribute is kept, and the text of each diagnostic, where it is emitted |
/// | [`max_depth`](Self::max_depth) | 256 | by a scan before parsing, at each start tag; the document element is depth 1 |
/// | [`max_nodes`](Self::max_nodes) | 1,000,000 | by the parser, for its nodes; by the scan, for the bindings in scope at the elements that declare one |
/// | [`max_namespace_bindings`](Self::max_namespace_bindings) | 256 | by the scan, at each start tag: one per prefix in scope, the default namespace counted, `xml` not |
///
/// The size, depth, binding and node limits bound what a read allocates. The
/// bindings in scope that the parser copies count against the node limit.
/// Everything else the read keeps besides the parsed tree counts against the
/// size limit, in one budget separate from the input's own size (ADR 0041 D2,
/// amendment 1):
///
/// - the namespace text that preserved content keeps: the bytes of each
///   namespace URI a kept name holds, and `prefix + URI + 10` for each
///   binding it keeps, counted as each element or attribute is kept
///   (`SACM-SEC-003/kept-namespaces`);
/// - the text of each diagnostic the read emits: the bytes of its text
///   arguments and of its affected ids, counted where it is emitted
///   (`SACM-SEC-003/kept-text`). Its rendered message, which repeats those
///   arguments (escaped, up to six times their size), is not counted on its
///   own. A few templates render an argument twice.
///
/// The read is refused, in both modes, at the element, attribute or
/// diagnostic where the budget passes the limit. So neither grows with
/// elements times bindings, or with ids times diagnostics, unchecked.
///
/// # Memory
///
/// The peak memory of a read at the defaults is recorded in
/// [`read`](super::read#limits).
///
/// # Examples
///
/// ```
/// use sacm::xmi::{self, Limit, Limits, ReadError, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
/// </sacm:ArgumentPackage>"#;
/// // The package is depth 1, the Claim 2 and its name 3.
/// let options = ReadOptions::default().with_limits(Limits::default().with_max_depth(2));
/// let error = xmi::read(input, &options).unwrap_err();
/// assert!(matches!(error, ReadError::LimitExceeded { limit: Limit::Depth, max: 2, .. }));
/// assert_eq!(error.location().map(|at| (at.line(), at.column())), Some((3, 54)));
/// assert_eq!(error.to_string(), "an element is nested deeper than the depth limit of 2 at line 3, column 54");
///
/// // With the default limits it reads.
/// assert!(xmi::read(input, &ReadOptions::default()).is_ok());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[expect(
    clippy::struct_field_names,
    reason = "each field is a maximum, named as its getter and setter are"
)]
pub struct Limits {
    /// The most bytes of input, and of what a read keeps besides the parsed
    /// tree.
    max_input_bytes: u64,
    /// The deepest element.
    max_depth: u64,
    /// The most XML nodes, and namespace copies.
    max_nodes: u64,
    /// The most bindings in scope at one element.
    max_namespace_bindings: u64,
}

impl Default for Limits {
    /// 33,554,432 bytes (32 MiB), depth 256, 1,000,000 nodes and 256
    /// bindings (M6 decision Q9; the size and node limits as the maintainer
    /// lowered them on 2026-11-19).
    fn default() -> Self {
        Self {
            max_input_bytes: 33_554_432,
            max_depth: 256,
            max_nodes: 1_000_000,
            max_namespace_bindings: 256,
        }
    }
}

impl Limits {
    /// These limits, refusing input of more than `bytes` bytes; what the read
    /// keeps besides the parsed tree, the namespace text of preserved content
    /// and the text of its diagnostics, is counted against the same limit, in
    /// one budget.
    #[must_use]
    pub fn with_max_input_bytes(mut self, bytes: u64) -> Self {
        self.max_input_bytes = bytes;
        self
    }

    /// These limits, refusing an element nested deeper than `depth`; the
    /// document element is depth 1.
    #[must_use]
    pub fn with_max_depth(mut self, depth: u64) -> Self {
        self.max_depth = depth;
        self
    }

    /// These limits, refusing input of more than `nodes` XML nodes (the
    /// document node, elements, text, comments and processing instructions),
    /// and namespace copies beyond it. At 4,294,967,295 or more, the parser's
    /// own capacity applies (SACM-SEC-002).
    #[must_use]
    pub fn with_max_nodes(mut self, nodes: u64) -> Self {
        self.max_nodes = nodes;
        self
    }

    /// These limits, refusing an element with more than `bindings` namespace
    /// bindings in scope: one per prefix, the default namespace counted,
    /// `xml` not.
    #[must_use]
    pub fn with_max_namespace_bindings(mut self, bindings: u64) -> Self {
        self.max_namespace_bindings = bindings;
        self
    }

    /// The most bytes of input, and of what a read keeps besides the parsed
    /// tree: the namespace text of preserved content and the text of its
    /// diagnostics.
    #[must_use]
    pub fn max_input_bytes(&self) -> u64 {
        self.max_input_bytes
    }

    /// The deepest an element may be nested; the document element is depth
    /// 1.
    #[must_use]
    pub fn max_depth(&self) -> u64 {
        self.max_depth
    }

    /// The most XML nodes the parser builds, and the most namespace bindings
    /// it copies.
    #[must_use]
    pub fn max_nodes(&self) -> u64 {
        self.max_nodes
    }

    /// The most namespace bindings in scope at one element.
    #[must_use]
    pub fn max_namespace_bindings(&self) -> u64 {
        self.max_namespace_bindings
    }

    /// The node limit to give roxmltree: `max_nodes`, at most `u32::MAX`.
    pub(super) fn parser_nodes(&self) -> u32 {
        u32::try_from(self.max_nodes).unwrap_or(u32::MAX)
    }

    /// Whether the node limit is below the parser's own capacity, so that
    /// the parser's node error is this limit's.
    pub(super) fn bounds_the_parser(&self) -> bool {
        self.max_nodes < u64::from(u32::MAX)
    }
}

/// Which of the [`Limits`] an input went over, as [`ReadError::LimitExceeded`]
/// reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
#[non_exhaustive]
pub enum Limit {
    /// [`Limits::max_input_bytes`]: the input's size, and what the read keeps
    /// besides the parsed tree, the namespace text of preserved content and
    /// the text of its diagnostics.
    InputBytes,
    /// [`Limits::max_depth`].
    Depth,
    /// [`Limits::max_nodes`]: the parser's nodes, and the namespace bindings
    /// it copies.
    Nodes,
    /// [`Limits::max_namespace_bindings`].
    NamespaceBindings,
}

impl fmt::Display for Limit {
    /// `input bytes`, `depth`, `nodes` or `namespace bindings`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InputBytes => "input bytes",
            Self::Depth => "depth",
            Self::Nodes => "nodes",
            Self::NamespaceBindings => "namespace bindings",
        })
    }
}

/// The refusal for `limit`, whose value is `max`, at `location`, with the
/// message of `template`: its `max`, after `size` for the input's size.
pub(super) fn exceeded(
    limit: Limit,
    max: u64,
    location: Option<SourceLocation>,
    template: &'static Template,
    size: Option<u64>,
) -> ReadError {
    let mut arguments = Vec::new();
    if let Some(size) = size {
        arguments.push(Argument::count("size", size));
    }
    arguments.push(Argument::count("max", max));
    ReadError::LimitExceeded {
        limit,
        max,
        location,
        details: Details::new(template, arguments),
    }
}

/// Refuses `input` if it is longer than the size limit: the first check of a
/// read, on the bytes.
pub(super) fn check_size(input: &[u8], limits: &Limits) -> Result<(), ReadError> {
    let size = u64::try_from(input.len()).unwrap_or(u64::MAX);
    if size > limits.max_input_bytes {
        return Err(exceeded(
            Limit::InputBytes,
            limits.max_input_bytes,
            None,
            &template::SEC_003_INPUT_BYTES,
            Some(size),
        ));
    }
    Ok(())
}

/// What a read keeps besides the parsed tree, counted in one budget against
/// the size limit (LIB-SEC-003; ADR 0041 D2, amendment 1): the namespace
/// text that preserved content keeps, each URI and binding as it is kept,
/// and the text of every diagnostic as it is emitted, which is the bytes of
/// its text arguments and of its affected ids. A diagnostic's rendered
/// message repeats its text arguments and is not counted on its own; a few
/// templates render an argument twice. The count never decreases.
pub(super) struct Budget<'t> {
    /// The text the read is of, which a refusal is located in.
    text: &'t str,
    /// The size limit.
    max: u64,
    /// The bytes kept so far.
    kept: Cell<u64>,
}

impl<'t> Budget<'t> {
    /// Nothing kept yet, in a read of `text` under `limits`.
    pub(super) fn new(text: &'t str, limits: &Limits) -> Self {
        Self {
            text,
            max: limits.max_input_bytes,
            kept: Cell::new(0),
        }
    }

    /// Counts `bytes` of namespace text that preserved content keeps at byte
    /// `offset`, and refuses it there (`SACM-SEC-003/kept-namespaces`) once
    /// the budget passes the limit.
    pub(super) fn keep_namespace_text(&self, bytes: u64, offset: usize) -> Result<(), ReadError> {
        self.keep(bytes, offset, &template::SEC_003_KEPT_NAMESPACES)
    }

    /// Counts the text of `diagnostic`, emitted at byte `offset`, and
    /// refuses it there (`SACM-SEC-003/kept-text`) once the budget passes the
    /// limit.
    pub(super) fn keep_diagnostic(
        &self,
        diagnostic: &Diagnostic,
        offset: usize,
    ) -> Result<(), ReadError> {
        self.keep(text_bytes(diagnostic), offset, &template::SEC_003_KEPT_TEXT)
    }

    /// Adds `bytes` at byte `offset`, refusing with `template` once the
    /// budget passes the limit.
    fn keep(
        &self,
        bytes: u64,
        offset: usize,
        template: &'static Template,
    ) -> Result<(), ReadError> {
        let kept = self.kept.get().saturating_add(bytes);
        self.kept.set(kept);
        if kept > self.max {
            return Err(exceeded(
                Limit::InputBytes,
                self.max,
                locate(self.text, offset),
                template,
                None,
            ));
        }
        Ok(())
    }

    /// The bytes kept so far.
    #[cfg(test)]
    pub(super) fn kept(&self) -> u64 {
        self.kept.get()
    }
}

/// The text of `diagnostic` that counts against the budget: the bytes of its
/// text arguments and of its affected ids.
fn text_bytes(diagnostic: &Diagnostic) -> u64 {
    let arguments = diagnostic
        .arguments()
        .iter()
        .filter_map(|argument| argument.value().as_text())
        .map(str::len);
    let affected = diagnostic.affected().iter().map(|id| id.as_str().len());
    let length = arguments
        .chain(affected)
        .fold(0_usize, usize::saturating_add);
    u64::try_from(length).unwrap_or(u64::MAX)
}

/// The namespace bindings in scope during the scan: how many declarations of
/// each prefix are open, and how many prefixes have one.
#[derive(Default)]
struct Scope<'t> {
    /// The open declarations of each prefix; `None` is the default
    /// namespace.
    open: HashMap<Option<&'t str>, u32>,
    /// The prefixes with at least one open declaration.
    distinct: u64,
}

impl<'t> Scope<'t> {
    /// Opens a declaration of `prefix`.
    fn declare(&mut self, prefix: Option<&'t str>) {
        let count = self.open.entry(prefix).or_insert(0);
        if *count == 0 {
            self.distinct += 1;
        }
        *count = count.saturating_add(1);
    }

    /// Closes a declaration of `prefix`.
    fn undeclare(&mut self, prefix: Option<&'t str>) {
        if let Some(count) = self.open.get_mut(&prefix) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.distinct = self.distinct.saturating_sub(1);
            }
        }
    }
}

/// The scan of [`scan`]: the depth, bindings and copies of `text` so far.
struct Scan<'t, 'l> {
    text: &'t str,
    limits: &'l Limits,
    /// The prefixes each open element declared, innermost last.
    frames: Vec<Vec<Option<&'t str>>>,
    scope: Scope<'t>,
    /// The bindings in scope at each element that declared one, so far.
    copies: u64,
}

impl<'t> Scan<'t, '_> {
    /// The refusal for `limit` at the `<` at byte `offset`.
    fn refuse(
        &self,
        limit: Limit,
        max: u64,
        offset: usize,
        template: &'static Template,
    ) -> ReadError {
        exceeded(limit, max, locate(self.text, offset), template, None)
    }

    /// Checks the element whose start tag at byte `offset` declares
    /// `declared`, then opens it: its depth, then the bindings in scope at
    /// it, then the copies.
    fn open(&mut self, offset: usize, declared: Vec<Option<&'t str>>) -> Result<(), ReadError> {
        let depth = u64::try_from(self.frames.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        if depth > self.limits.max_depth {
            return Err(self.refuse(
                Limit::Depth,
                self.limits.max_depth,
                offset,
                &template::SEC_003_DEPTH,
            ));
        }
        for &prefix in &declared {
            self.scope.declare(prefix);
        }
        if self.scope.distinct > self.limits.max_namespace_bindings {
            return Err(self.refuse(
                Limit::NamespaceBindings,
                self.limits.max_namespace_bindings,
                offset,
                &template::SEC_003_NAMESPACE_BINDINGS,
            ));
        }
        if !declared.is_empty() {
            self.copies = self.copies.saturating_add(self.scope.distinct);
            if self.copies > self.limits.max_nodes {
                return Err(self.refuse(
                    Limit::Nodes,
                    self.limits.max_nodes,
                    offset,
                    &template::SEC_003_NAMESPACE_COPIES,
                ));
            }
        }
        self.frames.push(declared);
        Ok(())
    }

    /// Closes the innermost open element; `false` if none is open.
    fn close(&mut self) -> bool {
        let Some(declared) = self.frames.pop() else {
            return false;
        };
        for prefix in declared {
            self.scope.undeclare(prefix);
        }
        true
    }
}

/// Checks the depth, the namespace bindings in scope and the namespace
/// copies of `text` against `limits`, before it is parsed, as the module
/// docs describe.
pub(super) fn scan(text: &str, limits: &Limits) -> Result<(), ReadError> {
    let mut scan = Scan {
        text,
        limits,
        frames: Vec::new(),
        scope: Scope::default(),
        copies: 0,
    };
    let mut offset = 0;
    while let Some(rest) = text.get(offset..).filter(|rest| !rest.is_empty()) {
        let length = if rest.starts_with("<!--") {
            past(rest, "<!--", "-->")
        } else if rest.starts_with("<![CDATA[") {
            past(rest, "<![CDATA[", "]]>")
        } else if rest.starts_with("<!") {
            // A DOCTYPE, which the parser refuses.
            None
        } else if rest.starts_with("<?") {
            past(rest, "<?", "?>")
        } else if rest.starts_with("</") {
            if scan.close() {
                rest.find('>').map(|end| end + 1)
            } else {
                None
            }
        } else if rest.starts_with('<') {
            match start_tag(rest) {
                Some((length, declared, empty)) => {
                    scan.open(offset, declared)?;
                    if empty {
                        scan.close();
                    }
                    Some(length)
                }
                None => None,
            }
        } else {
            Some(rest.find('<').unwrap_or(rest.len()))
        };
        // Every length is at least 1.
        let Some(length) = length else {
            break;
        };
        offset += length;
    }
    Ok(())
}

/// The length of the construct at the start of `rest`, which opens with
/// `open`, through the first `close` after it; `None` if it is not closed.
fn past(rest: &str, open: &str, close: &str) -> Option<usize> {
    let body = rest.get(open.len()..)?;
    body.find(close).map(|end| open.len() + end + close.len())
}

/// The start tag at the start of `rest`: its length through its `>`, the
/// prefixes it declares (`None` for the default namespace; `xml` left out),
/// and whether it is self-closing. `None` if it is not a well-formed tag the
/// scan can follow, which the parser then refuses.
fn start_tag(rest: &str) -> Option<(usize, Vec<Option<&str>>, bool)> {
    let after = rest.get(1..)?;
    let name = after
        .find(|c: char| is_xml_space(c) || matches!(c, '/' | '>'))
        .unwrap_or(after.len());
    if name == 0 {
        return None;
    }
    let mut index = 1 + name;
    let mut declared = Vec::new();
    loop {
        let tail = rest.get(index..)?;
        let trimmed = tail.trim_start_matches(is_xml_space);
        index += tail.len() - trimmed.len();
        if trimmed.starts_with('>') {
            return Some((index + 1, declared, false));
        }
        if trimmed.starts_with("/>") {
            return Some((index + 2, declared, true));
        }
        let length = trimmed.find(|c: char| is_xml_space(c) || matches!(c, '=' | '/' | '>'))?;
        if length == 0 {
            return None;
        }
        let attribute = trimmed.get(..length)?;
        index += length;
        let tail = rest.get(index..)?;
        let trimmed = tail.trim_start_matches(is_xml_space);
        index += tail.len() - trimmed.len();
        if !trimmed.starts_with('=') {
            return None;
        }
        index += 1;
        let tail = rest.get(index..)?;
        let trimmed = tail.trim_start_matches(is_xml_space);
        index += tail.len() - trimmed.len();
        let quote = trimmed
            .chars()
            .next()
            .filter(|&c| matches!(c, '"' | '\''))?;
        let value = trimmed.get(1..)?.find(quote)?;
        index += value + 2;
        if attribute == "xmlns" {
            declared.push(None);
        } else if let Some(prefix) = attribute.strip_prefix("xmlns:") {
            if prefix != "xml" {
                declared.push(Some(prefix));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Limits, scan, start_tag};

    #[test]
    fn a_start_tag_gives_its_length_declarations_and_whether_it_is_empty() {
        assert_eq!(
            start_tag("<a xmlns='u' xmlns:b=\">\" c=\"1\"/>x"),
            Some((32, vec![None, Some("b")], true))
        );
        assert_eq!(start_tag("<a xmlns:xml=\"u\">"), Some((17, vec![], false)));
        assert_eq!(start_tag("<a b>"), None);
        assert_eq!(start_tag("< a>"), None);
        assert_eq!(start_tag("<a b=\"1>"), None);
    }

    #[test]
    fn the_scan_stops_without_refusing_where_the_parser_refuses() {
        let tight = Limits::default().with_max_depth(1);
        for text in [
            "<a></b></c><d><e/></d>",
            "<a><!-- x",
            "<!DOCTYPE a><a><b/></a>",
            "<a b>",
        ] {
            assert!(scan(text, &tight).is_ok(), "{text}");
        }
        assert!(scan("<a/><b><c/></b>", &tight).is_err());
    }

    #[test]
    fn only_the_elements_that_declare_a_binding_add_copies() {
        // `r` declares one: 1 copy; `x` and `y` declare none, so add none.
        let one = Limits::default().with_max_nodes(1);
        assert!(scan("<r xmlns:a=\"u\"><x/><y/></r>", &one).is_ok());
        assert!(scan("<r xmlns:a=\"u\"><x xmlns:b=\"v\"/></r>", &one).is_err());
    }

    #[test]
    fn the_depth_is_checked_before_the_bindings_and_those_before_the_copies() {
        let template = |text: &str, limits: &Limits| scan(text, limits).err().map(|e| e.template());
        let both = "<a xmlns:b=\"u\"><c xmlns:d=\"v\"/></a>";
        let tight = Limits::default()
            .with_max_depth(1)
            .with_max_namespace_bindings(1)
            .with_max_nodes(1);
        assert_eq!(template(both, &tight), Some("SACM-SEC-003/depth"));
        assert_eq!(
            template(both, &tight.with_max_depth(2)),
            Some("SACM-SEC-003/namespace-bindings")
        );
        assert_eq!(
            template(
                both,
                &tight.with_max_depth(2).with_max_namespace_bindings(2)
            ),
            Some("SACM-SEC-003/namespace-copies")
        );
    }

    #[test]
    fn a_diagnostics_text_is_its_text_arguments_and_affected_ids() {
        use crate::diagnostic::template;
        use crate::diagnostic::{Argument, Diagnostic, Severity};
        use crate::model::ElementId;
        let diagnostic = Diagnostic::emit(
            &template::XMI_003_ELEMENT_KEPT,
            Severity::Warning,
            "LIB-COMPAT-003",
            vec![
                Argument::text("element", "v:x"),
                Argument::text("holder_class", "Claim"),
                Argument::text("holder_id", "c1"),
            ],
        )
        .set_affected([ElementId::new("c1")]);
        assert_eq!(super::text_bytes(&diagnostic), 3 + 5 + 2 + 2);
        // A count argument is no text.
        let counted = Diagnostic::emit(
            &template::SEC_003_KEPT_TEXT,
            Severity::Error,
            "LIB-SEC-003",
            vec![Argument::count("max", 123_456)],
        );
        assert_eq!(super::text_bytes(&counted), 0);
        // The budget refuses at the first count that passes the limit.
        let text = "<a/>";
        let budget = super::Budget::new(text, &Limits::default().with_max_input_bytes(12));
        assert!(budget.keep_diagnostic(&diagnostic, 0).is_ok());
        assert!(budget.keep_namespace_text(1, 0).is_err());
        assert_eq!(budget.kept(), 13);
    }

    #[test]
    fn a_node_limit_of_u32_max_or_more_is_the_parsers_capacity() {
        let at = |nodes: u64| Limits::default().with_max_nodes(nodes);
        let below = u64::from(u32::MAX) - 1;
        assert!(at(below).bounds_the_parser());
        assert_eq!(at(below).parser_nodes(), u32::MAX - 1);
        assert!(!at(u64::from(u32::MAX)).bounds_the_parser());
        assert!(!at(u64::MAX).bounds_the_parser());
        assert_eq!(at(u64::MAX).parser_nodes(), u32::MAX);
    }
}
