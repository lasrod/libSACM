//! The input layer of the XMI reader: bytes to a well-formed XML tree, or a
//! refusal (LIB-SEC-001, LIB-SEC-002, LIB-SEC-005, LIB-SEC-006, LIB-SEC-008,
//! LIB-API-009, SACM23-XMI-018; ADR 0005, ADR 0015, ADR 0016).
//!
//! [`read`](super::read) first refuses input longer than its size limit, on
//! the bytes (LIB-SEC-003). Then the steps here run in this order:
//! 1. UTF-16 with a byte order mark is decoded with std, keeping the mark as
//!    a leading U+FEFF; the steps below then run on the decoded text. UCS-4
//!    (UTF-32), and UTF-16 without a mark, are `Unsupported`, recognised by
//!    the mark or by the first bytes of `<?xml` (XML 1.0 Appendix F).
//! 2. The XML declaration's `<?xml`, `VersionInfo` and `EncodingDecl` must match
//!    XML 1.0 [23]-[26], [80] and [81], or the input is `Malformed`. The
//!    declaration is read from the longest valid prefix, so a Latin-1 file is
//!    refused for its encoding, not as malformed.
//! 3. Without a byte order mark, a declared `ISO-8859-1`, in any ASCII
//!    letter case and with no alias, decodes the whole input byte for byte,
//!    each byte as the scalar value of the same number, and the declaration
//!    is checked again on that text (LIB-COMPAT-016, ADR 0041 D7); after a
//!    UTF-8 mark it is `Malformed`, as it contradicts the mark. Without a
//!    UTF-16 mark, a declared encoding other than those, UTF-8 and ASCII is
//!    `Unsupported`. After one, a name other than UTF-16, UTF-16LE or
//!    UTF-16BE is `Malformed`. For an accepted name, the rest of the
//!    declaration (`SDDecl` [32] and `?>`) must match too.
//! 4. Under an ASCII declaration, a non-ASCII character is `Malformed`.
//! 5. Invalid UTF-8 is `Malformed`, at the first invalid byte. In UTF-16, an
//!    unpaired surrogate is `Malformed` at the surrogate, and an odd byte
//!    count after the last whole character. ISO-8859-1 has no invalid byte.
//! 6. The scan of `limits` checks the depth, the namespace bindings in scope,
//!    the namespace copies (LIB-SEC-003) and the attributes of each element
//!    (LIB-SEC-007); roxmltree then parses with
//!    DTDs refused and the node limit: a DOCTYPE is `Doctype`, the node
//!    limit `LimitExceeded`, and any other error is `Malformed`.
//! 7. What roxmltree accepts but XML 1.0 or Namespaces 1.0 make a fatal error
//!    is `Malformed`, at the earliest offending construct: a character
//!    reference to a surrogate or above U+10FFFF (which roxmltree would turn
//!    into U+FFFD), a declaration of the prefix `xmlns` (`xmlns:xmlns`), a
//!    namespace declaration repeated in one start tag (`xmlns` or `xmlns:xml`
//!    twice; roxmltree refuses other repeated prefixes itself), a prefix
//!    undeclared with `xmlns:p=""`, an element or attribute name with
//!    an empty prefix (`:c`), and a processing instruction whose target (the
//!    XML Name after `<?`) is `xml` in any letter case, contains a colon, or
//!    is followed by neither white space nor `?>`. Within the W3C XML
//!    Conformance Test Suite (20130923), this list is closed: every
//!    not-well-formed XML 1.0 and Namespaces 1.0 case in it is refused
//!    (LIB-SEC-006). Outside the suite, other roxmltree leniencies may
//!    remain: LIB-SEC-005's sweep checks only that this scan agrees with
//!    roxmltree's tree.
//!
//! roxmltree stays private to the `xmi` module: its errors become our own
//! reasons and a [`SourceLocation`].
//!
//! Every location comes from a `Locator` (or `locate`, for one offset), which
//! ends lines as XML 1.0 §2.11 does: a line feed, a carriage return and line
//! feed, or a lone carriage return each end one line. A leading byte order
//! mark (U+FEFF) takes no column. A `Locator` advances, so the many
//! locations of one read cost one pass over the text. roxmltree counts only
//! line feeds, and counts the mark as a character, so its positions are
//! converted to byte offsets first.

use std::borrow::Cow;
use std::collections::HashSet;

use crate::diagnostic::template::{self, Template};
use crate::diagnostic::{Argument, Details, SourceLocation};

use super::refusal::{Unread, unsupported, unsupported_at};
use super::{Limit, Limits, ReadError, limits};

/// The details of a message of `template`, made from `arguments`.
fn details(template: &'static Template, arguments: Vec<Argument>) -> Details {
    Details::new(template, arguments)
}

/// The details of a message of `template`, whose one argument `name` is
/// `value`.
fn details_of(template: &'static Template, name: &'static str, value: &str) -> Details {
    Details::new(template, vec![Argument::text(name, value)])
}

/// The details of `template` with the one Prose argument `name` = `value`.
fn prose_of(template: &'static Template, name: &'static str, value: &str) -> Details {
    Details::new(template, vec![Argument::prose(name, value)])
}

/// The text of `input`, if it is UTF-8 (or ASCII, if declared so), UTF-16
/// with a byte order mark, or ISO-8859-1 without a mark, its XML declaration
/// is well-formed, and it declares no encoding the reader does not support.
/// UTF-16 text is decoded into a new string that keeps the mark as a leading
/// U+FEFF; ISO-8859-1 text into a new string, a character per byte.
pub(super) fn decode(input: &[u8]) -> Result<Cow<'_, str>, ReadError> {
    let (text, encoding, decode_error) = match encoding_by_bom(input) {
        Some(Bom::Utf16(big_endian)) => {
            let (text, error) = decode_utf16(input, big_endian);
            (Cow::Owned(text), Encoding::Utf16, error)
        }
        Some(Bom::Other(encoding)) => {
            return Err(unsupported_at(
                SourceLocation::at(1, 1),
                Unread::Encoding,
                prose_of(&template::XML_002_BYTE_ORDER_MARK, "encoding", encoding),
            ));
        }
        None => {
            if let Some(encoding) = encoding_by_first_bytes(input) {
                return Err(unsupported_at(
                    SourceLocation::at(1, 1),
                    Unread::Encoding,
                    prose_of(&template::XML_002_DETECTED_ENCODING, "encoding", encoding),
                ));
            }
            let (text, error) = decode_utf8(input);
            (Cow::Borrowed(text), Encoding::Utf8, error)
        }
    };
    match check_declaration(&text, encoding, decode_error.as_ref())? {
        Checked::Latin1 => {
            let latin1: String = input.iter().copied().map(char::from).collect();
            check_declaration(&latin1, Encoding::Latin1, None)?;
            return Ok(Cow::Owned(latin1));
        }
        Checked::Declared(Some(declared)) => check_ascii(&text, &declared)?,
        Checked::Declared(None) => {}
    }
    match decode_error {
        Some(error) => Err(error),
        None => Ok(text),
    }
}

/// The encoding `decode` reads the input in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    /// UTF-8, or ASCII if declared so.
    Utf8,
    /// UTF-16 with a byte order mark, in either byte order.
    Utf16,
    /// ISO-8859-1, which a declaration named in input read as UTF-8.
    Latin1,
}

/// The name of ISO-8859-1, which a declaration may give in any ASCII letter
/// case (XML 1.0 §4.3.3), with no alias (M6 decision Q10).
const LATIN_1: &str = "ISO-8859-1";

/// The byte order mark, U+FEFF, as a leading character of decoded text.
const BOM: char = '\u{FEFF}';

/// What the XML declaration says of the encoding.
#[derive(Debug)]
enum Checked<'a> {
    /// The encoding it declares, if any, in which the input is read.
    Declared(Option<Declared<'a>>),
    /// ISO-8859-1, in input without a byte order mark: the input is read
    /// again in it.
    Latin1,
}

/// Whether a declared encoding is read as the input was decoded, or the
/// input is read again as ISO-8859-1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Support {
    Read,
    Latin1,
}

/// The longest valid UTF-8 prefix of `input`, and the refusal of the
/// invalid byte after it, if there is one.
fn decode_utf8(input: &[u8]) -> (&str, Option<ReadError>) {
    match std::str::from_utf8(input) {
        Ok(text) => (text, None),
        Err(error) => {
            // The bytes up to `valid_up_to` are valid UTF-8, so the fallback
            // is never taken.
            let text = input
                .get(..error.valid_up_to())
                .and_then(|valid| std::str::from_utf8(valid).ok())
                .unwrap_or_default();
            (text, Some(invalid_utf8(text)))
        }
    }
}

/// The longest valid prefix of `input`, UTF-16 in the byte order given, with
/// its byte order mark as a leading U+FEFF, and the refusal of what follows
/// it, if anything: an unpaired surrogate, or a last byte that is not a
/// whole code unit.
fn decode_utf16(input: &[u8], big_endian: bool) -> (String, Option<ReadError>) {
    let pairs = input.chunks_exact(2);
    let odd = !pairs.remainder().is_empty();
    let units = pairs.map(|pair| {
        // `chunks_exact(2)` yields only slices of two bytes.
        let bytes = [pair[0], pair[1]];
        if big_endian {
            u16::from_be_bytes(bytes)
        } else {
            u16::from_le_bytes(bytes)
        }
    });
    let mut text = String::with_capacity(input.len());
    for c in char::decode_utf16(units) {
        match c {
            Ok(c) => text.push(c),
            Err(error) => {
                let reason = details(
                    &template::XML_001_UNPAIRED_SURROGATE,
                    vec![Argument::count(
                        "unit",
                        u64::from(error.unpaired_surrogate()),
                    )],
                );
                let error = malformed(&text, text.len(), reason);
                return (text, Some(error));
            }
        }
    }
    let error = odd.then(|| {
        malformed(
            &text,
            text.len(),
            details(&template::XML_001_ODD_UTF16_BYTE, Vec::new()),
        )
    });
    (text, error)
}

/// Parses `text` as XML 1.0 with namespaces, with DTDs refused and the node
/// limit of `limits`, and refuses what roxmltree accepts but XML 1.0 or
/// Namespaces 1.0 do not.
pub(super) fn parse<'t>(
    text: &'t str,
    limits: &Limits,
) -> Result<roxmltree::Document<'t>, ReadError> {
    // DTDs are disabled by default, so a DOCTYPE is refused before its
    // internal subset is read (ADR 0005); the LIB-SEC-001 tests prove it.
    let options = roxmltree::ParsingOptions {
        nodes_limit: limits.parser_nodes(),
        ..roxmltree::ParsingOptions::default()
    };
    let document = roxmltree::Document::parse_with_options(text, options)
        .map_err(|error| refusal(text, &error, limits))?;
    check_unrepaired(text)?;
    Ok(document)
}

/// The refusal of invalid UTF-8 after `text`, the longest valid prefix.
fn invalid_utf8(text: &str) -> ReadError {
    malformed(
        text,
        text.len(),
        details(&template::XML_001_INVALID_UTF8, Vec::new()),
    )
}

/// A `Malformed` refusal at byte `offset` of `text`, whose reason `reason`
/// gives.
fn malformed(text: &str, offset: usize, reason: Details) -> ReadError {
    ReadError::malformed(locate(text, offset), reason)
}

/// A UTF-16 or UCS-4 byte order mark.
#[derive(Debug, PartialEq, Eq)]
enum Bom {
    /// UTF-16, big-endian if `true`.
    Utf16(bool),
    /// A UCS-4 (UTF-32) mark, with the name of its encoding.
    Other(&'static str),
}

/// The UTF-16 or UCS-4 byte order mark at the start of `input`, if any
/// (XML 1.0 Appendix F).
fn encoding_by_bom(input: &[u8]) -> Option<Bom> {
    // The four-byte marks first: two of them start with a UTF-16 mark.
    match input {
        [0x00, 0x00, 0xFE, 0xFF, ..] => Some(Bom::Other("UTF-32 (big-endian)")),
        [0xFF, 0xFE, 0x00, 0x00, ..] => Some(Bom::Other("UTF-32 (little-endian)")),
        [0x00, 0x00, 0xFF, 0xFE, ..] => Some(Bom::Other("UCS-4 (byte order 2143)")),
        [0xFE, 0xFF, 0x00, 0x00, ..] => Some(Bom::Other("UCS-4 (byte order 3412)")),
        [0xFE, 0xFF, ..] => Some(Bom::Utf16(true)),
        [0xFF, 0xFE, ..] => Some(Bom::Utf16(false)),
        _ => None,
    }
}

/// The encoding of input without a byte order mark that starts with `<` or
/// `<?` in UTF-16 or UCS-4 (XML 1.0 Appendix F). Such input can be valid
/// UTF-8, since every byte may be ASCII, but a `<` followed by NUL is never
/// well-formed XML in UTF-8.
fn encoding_by_first_bytes(input: &[u8]) -> Option<&'static str> {
    match input {
        [0x00, 0x00, 0x00, 0x3C, ..] => Some("UTF-32 (big-endian)"),
        [0x3C, 0x00, 0x00, 0x00, ..] => Some("UTF-32 (little-endian)"),
        [0x00, 0x00, 0x3C, 0x00, ..] => Some("UCS-4 (byte order 2143)"),
        [0x00, 0x3C, 0x00, 0x00, ..] => Some("UCS-4 (byte order 3412)"),
        [0x00, 0x3C, 0x00, 0x3F, ..] => Some("UTF-16 (big-endian)"),
        [0x3C, 0x00, 0x3F, 0x00, ..] => Some("UTF-16 (little-endian)"),
        _ => None,
    }
}

/// The encoding an XML declaration names.
#[derive(Debug)]
struct Declared<'a> {
    /// The name, as written.
    name: &'a str,
    /// The byte offset of the name in the text.
    offset: usize,
}

/// Whether `declared`, in any letter case, is read in `encoding`: UTF-8 or
/// ASCII in UTF-8 input, or ISO-8859-1 there without a byte order mark,
/// which reads the input again; UTF-16, UTF-16LE or UTF-16BE in UTF-16
/// input. Refuses another name in UTF-8 input (`Unsupported`), ISO-8859-1
/// after a UTF-8 mark and another name in UTF-16 input (`Malformed`: it
/// contradicts the byte order mark).
fn check_supported(
    text: &str,
    encoding: Encoding,
    declared: &Declared<'_>,
) -> Result<Support, ReadError> {
    let name = declared.name;
    let names: &[&str] = match encoding {
        Encoding::Utf8 => &["UTF-8", "US-ASCII", "ASCII"],
        Encoding::Utf16 => &["UTF-16", "UTF-16LE", "UTF-16BE"],
        Encoding::Latin1 => &[LATIN_1],
    };
    if names
        .iter()
        .any(|supported| name.eq_ignore_ascii_case(supported))
    {
        return Ok(Support::Read);
    }
    match encoding {
        Encoding::Utf8 if name.eq_ignore_ascii_case(LATIN_1) => {
            if text.starts_with(BOM) {
                Err(malformed(
                    text,
                    declared.offset,
                    details_of(
                        &template::XML_001_ENCODING_CONTRADICTS_UTF8_BOM,
                        "encoding",
                        name,
                    ),
                ))
            } else {
                Ok(Support::Latin1)
            }
        }
        Encoding::Utf8 | Encoding::Latin1 => Err(unsupported(
            text,
            declared.offset,
            Unread::Encoding,
            details_of(&template::XML_002_DECLARED_ENCODING, "encoding", name),
        )),
        Encoding::Utf16 => Err(malformed(
            text,
            declared.offset,
            details_of(
                &template::XML_001_ENCODING_CONTRADICTS_BOM,
                "encoding",
                name,
            ),
        )),
    }
}

/// Refuses `text` if `declared`, a supported encoding, is ASCII and `text`
/// holds a non-ASCII character.
fn check_ascii(text: &str, declared: &Declared<'_>) -> Result<(), ReadError> {
    let name = declared.name;
    if !["US-ASCII", "ASCII"]
        .iter()
        .any(|ascii| name.eq_ignore_ascii_case(ascii))
    {
        return Ok(());
    }
    match text.char_indices().find(|&(_, c)| !c.is_ascii()) {
        Some((offset, c)) => Err(malformed(
            text,
            offset,
            details(
                &template::XML_001_NON_ASCII_CHARACTER,
                vec![
                    Argument::text("encoding", name),
                    Argument::count("code_point", u64::from(u32::from(c))),
                ],
            ),
        )),
        None => Ok(()),
    }
}

/// Whether `c` is XML white space (production S).
pub(super) fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// The text after an optional leading byte order mark (U+FEFF), and its byte
/// offset.
fn after_bom(text: &str) -> (usize, &str) {
    match text.strip_prefix('\u{FEFF}') {
        Some(rest) => ('\u{FEFF}'.len_utf8(), rest),
        None => (0, text),
    }
}

/// The byte offset of the XML declaration in `text`, if it has one: `<?xml`
/// and white space at the start, after an optional byte order mark.
fn declaration_start(text: &str) -> Option<usize> {
    let (start, rest) = after_bom(text);
    rest.strip_prefix("<?xml")?
        .starts_with(is_xml_space)
        .then_some(start)
}

/// The pseudo-attributes of the XML declaration, in the order XML 1.0 [23]
/// requires.
const PSEUDO_ATTRIBUTES: [&str; 3] = ["version", "encoding", "standalone"];

/// Checks the XML declaration at the start of `text`, if it has one, and
/// returns the encoding it declares (XML 1.0 [23]-[26], [32], [80], [81]).
///
/// `<?xml`, `VersionInfo` and `EncodingDecl` are checked first, then whether the
/// encoding is supported in `encoding`, and only then `SDDecl` and `?>`.
/// `truncated` is the decoding error after `text`, if `text` is the valid
/// prefix of a longer input: a declaration that `text` ends inside is then
/// refused with it.
fn check_declaration<'a>(
    text: &'a str,
    encoding: Encoding,
    truncated: Option<&ReadError>,
) -> Result<Checked<'a>, ReadError> {
    let Some(start) = declaration_start(text) else {
        return Ok(Checked::Declared(None));
    };
    let mut cursor = Cursor {
        text,
        offset: start + "<?xml".len(),
        truncated,
    };
    // VersionInfo [24]: the white space after `<?xml` is there.
    cursor.skip_space();
    let (offset, name) = cursor.name();
    if name != "version" {
        return Err(cursor.misplaced(offset, name, true, "\"version\""));
    }
    let (offset, version) = cursor.value()?;
    if !is_version_number(version) {
        return Err(malformed(
            text,
            offset,
            details_of(&template::XML_001_XML_VERSION, "version", version),
        ));
    }
    // EncodingDecl [80], then the supported-encoding check.
    let mut spaced = cursor.skip_space();
    let mut declared_encoding = None;
    if spaced && cursor.peek_name() == "encoding" {
        cursor.name();
        let (offset, name) = cursor.value()?;
        if !is_encoding_name(name) {
            return Err(malformed(
                text,
                offset,
                details_of(&template::XML_001_ENCODING_NAME, "encoding", name),
            ));
        }
        let declared = Declared { name, offset };
        if check_supported(text, encoding, &declared)? == Support::Latin1 {
            return Ok(Checked::Latin1);
        }
        declared_encoding = Some(declared);
        spaced = cursor.skip_space();
    }
    // SDDecl [32].
    if spaced && cursor.peek_name() == "standalone" {
        cursor.name();
        let (offset, standalone) = cursor.value()?;
        if !matches!(standalone, "yes" | "no") {
            return Err(malformed(
                text,
                offset,
                details_of(&template::XML_001_STANDALONE, "value", standalone),
            ));
        }
        spaced = cursor.skip_space();
    }
    if cursor.rest().starts_with("?>") {
        return Ok(Checked::Declared(declared_encoding));
    }
    let (offset, name) = cursor.name();
    Err(cursor.misplaced(offset, name, spaced, "\"?>\""))
}

/// Whether `value` matches `VersionNum` [26]: `1.` and at least one digit.
fn is_version_number(value: &str) -> bool {
    value
        .strip_prefix("1.")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `value` matches `EncName` [81]: a Latin letter, then Latin letters,
/// digits, `.`, `_` and `-`.
fn is_encoding_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// A position in the XML declaration being checked.
struct Cursor<'a, 'e> {
    /// The whole text.
    text: &'a str,
    /// The byte offset of the next character.
    offset: usize,
    /// The decoding error after `text`, if `text` is the valid prefix of a
    /// longer input.
    truncated: Option<&'e ReadError>,
}

impl<'a> Cursor<'a, '_> {
    /// The text from the cursor on.
    fn rest(&self) -> &'a str {
        self.text.get(self.offset..).unwrap_or_default()
    }

    /// Skips white space, and says whether there was any.
    fn skip_space(&mut self) -> bool {
        let rest = self.rest();
        let skipped = rest.len() - rest.trim_start_matches(is_xml_space).len();
        self.offset += skipped;
        skipped > 0
    }

    /// The pseudo-attribute name at the cursor, without moving: everything
    /// up to white space, `=`, `?`, `>`, `<` or a quote. Empty if none.
    fn peek_name(&self) -> &'a str {
        let rest = self.rest();
        let end = rest
            .find(|c: char| is_xml_space(c) || matches!(c, '=' | '?' | '>' | '<' | '"' | '\''))
            .unwrap_or(rest.len());
        rest.get(..end).unwrap_or_default()
    }

    /// The pseudo-attribute name at the cursor and its offset, moving past
    /// it.
    fn name(&mut self) -> (usize, &'a str) {
        let offset = self.offset;
        let name = self.peek_name();
        self.offset += name.len();
        (offset, name)
    }

    /// The quoted value after `Eq` [25] at the cursor, and the offset of its
    /// first character, moving past its closing quote.
    fn value(&mut self) -> Result<(usize, &'a str), ReadError> {
        self.skip_space();
        if !self.rest().starts_with('=') {
            return Err(self.unexpected("\"=\""));
        }
        self.offset += 1;
        self.skip_space();
        let Some(quote) = self
            .rest()
            .chars()
            .next()
            .filter(|&c| matches!(c, '"' | '\''))
        else {
            return Err(self.unexpected("a quote"));
        };
        let start = self.offset + 1;
        let body = self.text.get(start..).unwrap_or_default();
        let Some(length) = body.find(quote) else {
            return Err(self.ended());
        };
        self.offset = start + length + 1;
        Ok((start, body.get(..length).unwrap_or_default()))
    }

    /// The refusal of the pseudo-attribute `name` at `offset`, where
    /// `expected` was expected. `spaced` says whether white space came
    /// before it. A name that a decoding error cuts short is refused with
    /// that error.
    fn misplaced(&self, offset: usize, name: &str, spaced: bool, expected: &str) -> ReadError {
        let reason = if name.is_empty() {
            return self.unexpected(expected);
        } else if self.truncated.is_some() && offset + name.len() == self.text.len() {
            return self.ended();
        } else if !PSEUDO_ATTRIBUTES.contains(&name) {
            details_of(&template::XML_001_UNKNOWN_PSEUDO_ATTRIBUTE, "name", name)
        } else if spaced {
            details_of(&template::XML_001_PSEUDO_ATTRIBUTE_ORDER, "name", name)
        } else {
            details_of(&template::XML_001_PSEUDO_ATTRIBUTE_SPACE, "name", name)
        };
        malformed(self.text, offset, reason)
    }

    /// The refusal of the character at the cursor, where `expected` was
    /// expected.
    fn unexpected(&self, expected: &str) -> ReadError {
        if self.rest().is_empty() {
            return self.ended();
        }
        malformed(
            self.text,
            self.offset,
            prose_of(
                &template::XML_001_DECLARATION_CHARACTER,
                "expected",
                expected,
            ),
        )
    }

    /// The refusal of a declaration that the text ends inside.
    fn ended(&self) -> ReadError {
        match self.truncated {
            Some(error) => error.clone(),
            None => malformed(
                self.text,
                self.text.len(),
                details(&template::XML_001_DECLARATION_END, Vec::new()),
            ),
        }
    }
}

/// Refuses the first construct in `text` that XML 1.0 or Namespaces 1.0
/// make a fatal error but roxmltree lets through (LIB-SEC-005, LIB-SEC-006):
/// a character reference to a surrogate or above U+10FFFF, a declaration of
/// the prefix `xmlns` (`xmlns:xmlns`, Namespaces 1.0 §3), a namespace
/// declaration repeated in one start tag (XML 1.0 §3.1, LIB-SEC-008), a prefix
/// undeclared with `xmlns:p=""`, an element or attribute name with an empty
/// prefix (`<:c>`, `</:c>`, ` :a="1"`), which roxmltree reads as unprefixed,
/// and a processing instruction whose target (the XML Name after `<?`,
/// [4]/[4a]) is `xml` in any letter case, contains a colon (Namespaces 1.0 §7),
/// or is followed by neither white space nor `?>` ([16]). The target checks run
/// in that order, and none applies to the checked declaration. Within the W3C
/// XML Conformance Test Suite, this list is closed (LIB-SEC-006). Outside it,
/// other roxmltree leniencies may remain: LIB-SEC-005's sweep checks only that
/// this scan agrees with roxmltree's tree.
///
/// It scans the text once, in document order, so the earliest construct is
/// refused. It scans the raw text rather than the tree, because roxmltree
/// merges adjacent text and CDATA into one node whose range covers only the
/// first. The XML declaration has been checked by [`decode`].
///
/// The scan stays in step with the markup because each construct ends at
/// the first closing delimiter after its opening one: a comment's text may
/// start with `>` or `->`, so `-->` is looked for after the `<!--`, not from
/// its `<`. roxmltree has accepted the text, so every comment, processing
/// instruction, CDATA section and tag is closed and every attribute value is
/// quoted, and the scan reaches the end of the text. It stops early, without
/// a refusal, only on text roxmltree refuses; that path exists so that no
/// input can make it panic.
fn check_unrepaired(text: &str) -> Result<(), ReadError> {
    let declaration = declaration_start(text);
    let mut offset = 0;
    while let Some(rest) = text.get(offset..).filter(|rest| !rest.is_empty()) {
        let length = if rest.starts_with("<!--") {
            closed(rest, "<!--", "-->")
        } else if rest.starts_with("<![CDATA[") {
            closed(rest, "<![CDATA[", "]]>")
        } else if let Some(body) = rest.strip_prefix("<?") {
            if Some(offset) != declaration {
                check_pi_target(text, offset, body)?;
            }
            closed(rest, "<?", "?>")
        } else if let Some(name) = rest.strip_prefix("</") {
            let end = name
                .find(|c: char| is_xml_space(c) || c == '>')
                .unwrap_or(name.len());
            check_name(
                text,
                offset + "</".len(),
                name.get(..end).unwrap_or_default(),
            )?;
            // After its name, an end tag holds only white space and `>`, so
            // the rest of it is scanned as character data, harmlessly.
            Some("</".len())
        } else if rest.starts_with('<') {
            start_tag(text, offset)?
        } else {
            let length = rest.find('<').unwrap_or(rest.len());
            check_references(text, offset, length)?;
            Some(length)
        };
        // Every length is at least 1: each construct is at least its opening
        // characters long, and character data at least one character.
        let Some(length) = length else {
            break;
        };
        offset += length;
    }
    Ok(())
}

/// Checks the target of the processing instruction at byte `offset` of
/// `text`, whose text after the `<?` is `body`: the XML Name there ([4]/[4a],
/// as roxmltree reads it) must not be `xml` in any letter case ([17]), must
/// not contain a colon (Namespaces 1.0 §7), and must be followed by white
/// space or `?>` ([16]), checked in that order.
fn check_pi_target(text: &str, offset: usize, body: &str) -> Result<(), ReadError> {
    let length = body.find(|c| !is_name_char(c)).unwrap_or(body.len());
    let target = body.get(..length).unwrap_or_default();
    if target.eq_ignore_ascii_case("xml") {
        return Err(malformed(
            text,
            offset,
            details_of(&template::XML_001_RESERVED_PI_TARGET, "target", target),
        ));
    }
    if target.contains(':') {
        return Err(malformed(
            text,
            offset,
            details_of(&template::XML_001_PI_TARGET_COLON, "target", target),
        ));
    }
    let after = body.get(length..).unwrap_or_default();
    if !(after.starts_with(is_xml_space) || after.starts_with("?>")) {
        return Err(malformed(
            text,
            offset + "<?".len() + length,
            details_of(&template::XML_001_PI_TARGET_END, "target", target),
        ));
    }
    Ok(())
}

/// Whether `c` is a `NameStartChar` (XML 1.0 [4]).
fn is_name_start_char(c: char) -> bool {
    matches!(c,
        ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}'
    )
}

/// Whether `c` is a `NameChar` (XML 1.0 [4a]).
fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || matches!(c,
            '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}'
        )
}

/// The length of the construct at the start of `rest`, which opens with
/// `open`, through the first `close` after `open`; `None` if it is not
/// closed.
fn closed(rest: &str, open: &str, close: &str) -> Option<usize> {
    let body = rest.get(open.len()..)?;
    body.find(close).map(|end| open.len() + end + close.len())
}

/// Refuses the element or attribute `name`, as written at byte `offset` of
/// `text`, if it has an empty prefix (`:c`), which roxmltree reads as no
/// prefix (Namespaces 1.0 §7, `QName`).
fn check_name(text: &str, offset: usize, name: &str) -> Result<(), ReadError> {
    if name.starts_with(':') {
        return Err(malformed(
            text,
            offset,
            details_of(&template::XML_001_EMPTY_PREFIX, "name", name),
        ));
    }
    Ok(())
}

/// Checks the start tag at byte `offset` of `text`: its element and
/// attribute names, its namespace declarations (none repeated in the tag),
/// and the character references in its attribute values.
/// Returns its length up to its closing `>` or `/>`, which the scan then
/// reads as character data, or `None` if it does not end.
fn start_tag(text: &str, offset: usize) -> Result<Option<usize>, ReadError> {
    let tag = text.get(offset..).unwrap_or_default();
    // Past the `<` and the element name.
    let mut index = tag
        .find(|c: char| is_xml_space(c) || matches!(c, '/' | '>'))
        .unwrap_or(tag.len());
    check_name(text, offset + 1, tag.get(1..index).unwrap_or_default())?;
    // The namespace declarations of this tag so far, by name as written.
    let mut declared = HashSet::new();
    loop {
        let Some(rest) = tag.get(index..) else {
            return Ok(None);
        };
        let trimmed = rest.trim_start_matches(is_xml_space);
        index += rest.len() - trimmed.len();
        // Outside a value, a `/` in a start tag starts its closing `/>`.
        if trimmed.starts_with(['>', '/']) {
            return Ok(Some(index));
        }
        // An attribute: its name, `Eq` and a quoted value.
        let Some(name_length) = trimmed.find(|c: char| is_xml_space(c) || c == '=') else {
            return Ok(None);
        };
        let name = trimmed.get(..name_length).unwrap_or_default();
        check_name(text, offset + index, name)?;
        // Namespaces 1.0 §3: the prefix `xmlns` "MUST NOT be declared".
        // roxmltree refuses only its binding to the xmlns namespace name, so
        // this comes before the undeclaring check: `xmlns:xmlns=""` is
        // refused for declaring `xmlns`.
        if name == "xmlns:xmlns" {
            return Err(malformed(
                text,
                offset + index,
                details(&template::XML_001_XMLNS_PREFIX_DECLARED, Vec::new()),
            ));
        }
        // XML 1.0 §3.1 Unique Att Spec: roxmltree refuses a repeated `xmlns:p`,
        // but not a repeated `xmlns` or `xmlns:xml`.
        if (name == "xmlns" || name.starts_with("xmlns:")) && !declared.insert(name) {
            let reason = match name.strip_prefix("xmlns:") {
                Some(prefix) => details_of(&template::XML_001_DUPLICATE_PREFIX, "prefix", prefix),
                None => details_of(&template::XML_001_DUPLICATE_ATTRIBUTE, "attribute", name),
            };
            return Err(malformed(text, offset + index, reason));
        }
        let Some(open) = trimmed.find(['"', '\'']) else {
            return Ok(None);
        };
        let quote = trimmed.get(open..).and_then(|q| q.chars().next());
        let value_start = index + open + 1;
        let Some(value_length) = quote.and_then(|q| tag.get(value_start..)?.find(q)) else {
            return Ok(None);
        };
        let undeclared = name.strip_prefix("xmlns:").filter(|_| value_length == 0);
        if let Some(prefix) = undeclared {
            return Err(malformed(
                text,
                offset + index,
                details_of(&template::XML_001_PREFIX_UNDECLARED, "prefix", prefix),
            ));
        }
        check_references(text, offset + value_start, value_length)?;
        index = value_start + value_length + 1;
    }
}

/// Refuses the first character reference in the `length` bytes at `offset`
/// of `text` that refers to no character: a surrogate, or a code point above
/// U+10FFFF (XML 1.0 §4.1, WFC Legal Character).
fn check_references(text: &str, offset: usize, length: usize) -> Result<(), ReadError> {
    let Some(segment) = text.get(offset..offset.saturating_add(length)) else {
        return Ok(());
    };
    for (index, _) in segment.match_indices("&#") {
        let Some(after) = segment.get(index + "&#".len()..) else {
            continue;
        };
        let (digits, radix) = match after.strip_prefix('x') {
            Some(hex) => (hex, 16),
            None => (after, 10),
        };
        let Some(end) = digits.find(';') else {
            continue;
        };
        let digits_only = digits.get(..end).unwrap_or_default();
        if digits_only.is_empty() || !digits_only.chars().all(|c| c.is_digit(radix)) {
            // Not a character reference; roxmltree refuses these.
            continue;
        }
        // Digits too many for `u32` are above U+10FFFF too.
        let value = u32::from_str_radix(digits_only, radix).ok();
        if value.and_then(char::from_u32).is_some() {
            continue;
        }
        let what = match value {
            Some(value) if value <= 0x10_FFFF => &template::XML_001_REFERENCE_TO_SURROGATE,
            _ => &template::XML_001_REFERENCE_ABOVE_MAX,
        };
        let reference = after
            .get(..after.len() - digits.len() + end + 1)
            .map(|body| format!("&#{body}"))
            .unwrap_or_default();
        return Err(malformed(
            text,
            offset + index,
            details_of(what, "reference", &reference),
        ));
    }
    Ok(())
}

/// The byte offset of the `<` of a DOCTYPE where roxmltree looks for one:
/// after the XML declaration, comments, processing instructions and white
/// space.
fn doctype_offset(text: &str) -> Option<usize> {
    let (mut offset, mut rest) = after_bom(text);
    if rest.starts_with("<?xml ") {
        let end = declaration_end(rest)?;
        offset += end;
        rest = rest.get(end..)?;
    }
    loop {
        let trimmed = rest.trim_start_matches(is_xml_space);
        offset += rest.len() - trimmed.len();
        rest = trimmed;
        let end = if let Some(body) = rest.strip_prefix("<!--") {
            "<!--".len() + body.find("-->")? + "-->".len()
        } else if let Some(body) = rest.strip_prefix("<?") {
            "<?".len() + body.find("?>")? + "?>".len()
        } else {
            break;
        };
        offset += end;
        rest = rest.get(end..)?;
    }
    rest.starts_with("<!DOCTYPE").then_some(offset)
}

/// The byte length of the XML declaration at the start of `text`, through its
/// `?>`. A `?>` inside a quoted value does not end it.
fn declaration_end(text: &str) -> Option<usize> {
    let mut quote = None;
    for (index, c) in text.char_indices() {
        match quote {
            Some(open) if c == open => quote = None,
            None if matches!(c, '"' | '\'') => quote = Some(c),
            None if c == '?' && text.get(index..)?.starts_with("?>") => {
                return Some(index + "?>".len());
            }
            Some(_) | None => {}
        }
    }
    None
}

/// The location of byte `offset` of `text`. Lines end as XML 1.0 §2.11 ends
/// them: at a line feed, a carriage return and line feed (once), or a lone
/// carriage return. Columns count characters; a leading byte order mark
/// (U+FEFF, from UTF-8 or UTF-16) is an encoding signature, not text, and
/// takes no column. `None` if
/// `offset` is not a character boundary of `text` or the position does not
/// fit in `u32`.
pub(super) fn locate(text: &str, offset: usize) -> Option<SourceLocation> {
    Locator::new(text).locate(offset)
}

/// Locates byte offsets of one text, as [`locate`] does, with a cursor that
/// advances: offsets asked for in increasing order cost one pass over the
/// text in all, so a read that locates many diagnostics stays linear. An
/// offset before the cursor restarts it from the beginning.
pub(super) struct Locator<'t> {
    text: &'t str,
    /// Where counting starts: after a leading byte order mark.
    start: usize,
    /// The byte offset counted up to, a character boundary.
    cursor: usize,
    /// The lines ended before the cursor.
    lines: usize,
    /// The characters between the start of the cursor's line and the cursor.
    column: usize,
    /// Whether the character before the cursor is a carriage return.
    after_cr: bool,
}

impl<'t> Locator<'t> {
    /// A locator for `text`, at its start.
    pub(super) fn new(text: &'t str) -> Self {
        let start = if text.starts_with('\u{FEFF}') {
            '\u{FEFF}'.len_utf8()
        } else {
            0
        };
        Self {
            text,
            start,
            cursor: start,
            lines: 0,
            column: 0,
            after_cr: false,
        }
    }

    /// The location of byte `offset`, as [`locate`] gives it.
    pub(super) fn locate(&mut self, offset: usize) -> Option<SourceLocation> {
        // A character boundary within the text; inside a leading mark only
        // offset 0 is one, and it is the start.
        self.text.get(..offset)?;
        let target = offset.max(self.start);
        if target < self.cursor {
            *self = Self::new(self.text);
        }
        let between = self.text.get(self.cursor..target)?;
        for c in between.chars() {
            match c {
                // The line feed of a CRLF: the CR already ended the line.
                '\n' if self.after_cr => {}
                '\n' | '\r' => {
                    self.lines = self.lines.saturating_add(1);
                    self.column = 0;
                }
                _ => self.column = self.column.saturating_add(1),
            }
            self.after_cr = c == '\r';
        }
        self.cursor = target;
        let line = u32::try_from(self.lines.checked_add(1)?).ok()?;
        let column = u32::try_from(self.column.checked_add(1)?).ok()?;
        SourceLocation::at(line, column)
    }
}

/// The byte offset in `text` of a roxmltree position. roxmltree counts rows
/// by line feeds only, and columns by characters after the last line feed.
/// `None` if the position is not in `text`.
fn offset_of(text: &str, position: roxmltree::TextPos) -> Option<usize> {
    let row = usize::try_from(position.row).ok()?.checked_sub(1)?;
    let column = usize::try_from(position.col).ok()?.checked_sub(1)?;
    let line_start = match row.checked_sub(1) {
        None => 0,
        Some(line_feeds_before) => text.match_indices('\n').nth(line_feeds_before)?.0 + 1,
    };
    let line = text.get(line_start..)?;
    let within = line
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(line.len()))
        .nth(column)?;
    Some(line_start + within)
}

/// The refusal for a roxmltree error. The location is roxmltree's, converted
/// to §2.11 lines, except for a DOCTYPE, which roxmltree does not locate,
/// and for the errors it places at a stand-in 1:1, which are located at the
/// end of the input. The node limit is `LimitExceeded`, with no location,
/// when `limits` set it below the parser's own capacity (LIB-SEC-003).
fn refusal(text: &str, error: &roxmltree::Error, limits: &Limits) -> ReadError {
    use roxmltree::Error as E;

    match error {
        E::DtdDetected => ReadError::Doctype {
            location: doctype_offset(text).and_then(|offset| locate(text, offset)),
        },
        E::NodesLimitReached if limits.bounds_the_parser() => limits::exceeded(
            Limit::Nodes,
            limits.max_nodes(),
            None,
            &template::SEC_003_NODES,
            None,
        ),
        E::NodesLimitReached | E::AttributesLimitReached | E::NamespacesLimitReached => {
            unsupported_at(None, Unread::Capacity, reason(error))
        }
        E::NoRootNode | E::UnclosedRootNode | E::UnexpectedEndOfStream => {
            ReadError::malformed(locate(text, text.len()), reason(error))
        }
        _ => ReadError::malformed(
            offset_of(text, error.pos()).and_then(|offset| locate(text, offset)),
            reason(error),
        ),
    }
}

/// Our own reason for a roxmltree error: its template and arguments. It
/// states no position, and it quotes names from the input with `{:?}`, so it
/// stays on one line. roxmltree's `Display` does neither. A DOCTYPE is
/// refused as `ReadError::Doctype` before its reason is asked for.
fn reason(error: &roxmltree::Error) -> Details {
    use roxmltree::Error as E;

    let none = |template| details(template, Vec::new());
    match error {
        E::NodesLimitReached => none(&template::SEC_002_NODES),
        E::AttributesLimitReached => none(&template::SEC_002_ATTRIBUTES),
        E::NamespacesLimitReached => none(&template::SEC_002_NAMESPACES),
        E::NoRootNode => none(&template::XML_001_NO_ROOT),
        E::UnclosedRootNode => none(&template::XML_001_UNCLOSED_ROOT),
        E::DtdDetected => none(&template::SEC_001_DOCTYPE),
        E::UnexpectedEndOfStream => none(&template::XML_001_UNEXPECTED_END),
        E::InvalidXmlPrefixUri(_) => none(&template::XML_001_XML_PREFIX_URI),
        E::UnexpectedXmlUri(_) => none(&template::XML_001_XML_URI_PREFIX),
        E::UnexpectedXmlnsUri(_) => none(&template::XML_001_XMLNS_URI),
        E::InvalidElementNamePrefix(_) => none(&template::XML_001_XMLNS_ELEMENT_PREFIX),
        E::DuplicatedNamespace(prefix, _) => {
            details_of(&template::XML_001_DUPLICATE_PREFIX, "prefix", prefix)
        }
        E::UnknownNamespace(prefix, _) => {
            details_of(&template::XML_001_UNKNOWN_PREFIX, "prefix", prefix)
        }
        E::UnexpectedCloseTag(expected, actual, _) => details(
            &template::XML_001_END_TAG_MISMATCH,
            vec![
                Argument::text("end_tag", actual.as_str()),
                Argument::text("start_tag", expected.as_str()),
            ],
        ),
        E::UnexpectedEntityCloseTag(_) => none(&template::XML_001_ENTITY_CLOSES_ELEMENT),
        E::UnknownEntityReference(name, _) => {
            details_of(&template::XML_001_UNKNOWN_ENTITY, "entity", name)
        }
        E::MalformedEntityReference(_) => none(&template::XML_001_MALFORMED_REFERENCE),
        E::EntityReferenceLoop(_) => none(&template::XML_001_ENTITY_LOOP),
        E::InvalidAttributeValue(_) => none(&template::XML_001_LT_IN_ATTRIBUTE_VALUE),
        E::DuplicatedAttribute(name, _) => {
            details_of(&template::XML_001_DUPLICATE_ATTRIBUTE, "attribute", name)
        }
        E::UnexpectedDeclaration(_) => none(&template::XML_001_MISPLACED_DECLARATION),
        E::InvalidName(_) => none(&template::XML_001_INVALID_NAME),
        E::NonXmlChar(c, _) => details(
            &template::XML_001_NON_XML_CHARACTER,
            vec![Argument::count("code_point", u64::from(u32::from(*c)))],
        ),
        E::InvalidChar(expected, _, _) => prose_of(
            &template::XML_001_UNEXPECTED_CHARACTER,
            "expected",
            &format!("{:?}", char::from(*expected).to_string()),
        ),
        // roxmltree's descriptions are fixed English phrases that
        // single-quote characters, as in "'>'".
        E::InvalidChar2(expected, _, _) => prose_of(
            &template::XML_001_UNEXPECTED_CHARACTER,
            "expected",
            &expected
                .replace("a whitespace", "white space")
                .replace('\'', "\""),
        ),
        E::InvalidString(expected, _) => {
            details_of(&template::XML_001_EXPECTED_STRING, "expected", expected)
        }
        E::InvalidExternalID(_) => none(&template::XML_001_INVALID_EXTERNAL_ID),
        E::EntityResolver(..) => none(&template::XML_001_UNRESOLVED_ENTITY),
        E::InvalidComment(_) => none(&template::XML_001_INVALID_COMMENT),
        E::InvalidCharacterData(_) => none(&template::XML_001_CDATA_END_IN_TEXT),
        E::UnknownToken(_) => none(&template::XML_001_UNKNOWN_TOKEN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Code;

    #[test]
    fn locate_counts_lines_and_characters() {
        assert_eq!(locate("", 0), SourceLocation::at(1, 1));
        assert_eq!(locate("a\né\nbc", 7), SourceLocation::at(3, 3));
        assert_eq!(locate("é", 1), None, "not a character boundary");
        assert_eq!(locate("ab", 3), None, "past the end");
        // XML 1.0 §2.11: LF, CRLF and a lone CR each end one line.
        assert_eq!(locate("a\rb", 2), SourceLocation::at(2, 1));
        assert_eq!(locate("a\r\nb", 3), SourceLocation::at(2, 1));
        assert_eq!(locate("a\r\nb", 2), SourceLocation::at(2, 1), "at the LF");
        assert_eq!(locate("\r\r\n\n\rxy", 6), SourceLocation::at(5, 2));
        // A leading UTF-8 BOM takes no column; a later U+FEFF does.
        assert_eq!(locate("\u{FEFF}", 0), SourceLocation::at(1, 1));
        assert_eq!(locate("\u{FEFF}<r", 3), SourceLocation::at(1, 1));
        assert_eq!(locate("\u{FEFF}<r", 5), SourceLocation::at(1, 3));
        assert_eq!(locate("a\u{FEFF}b", 4), SourceLocation::at(1, 3));
    }

    #[test]
    fn a_locator_gives_locate_s_answer_for_offsets_in_any_order() {
        let texts = ["\u{FEFF}a\r\nbé\rc\n\nd\r", "x\r\r\n\ny", "", "\u{FEFF}"];
        for text in texts {
            let offsets: Vec<usize> = (0..=text.len() + 1).collect();
            // Forwards, backwards, and back and forth.
            let mut orders = vec![offsets.clone(), offsets.iter().rev().copied().collect()];
            orders.push(
                offsets
                    .iter()
                    .flat_map(|&offset| [offset, 0, offset, text.len()])
                    .collect(),
            );
            for order in orders {
                let mut locator = Locator::new(text);
                for offset in order {
                    assert_eq!(
                        locator.locate(offset),
                        locate_by_scan(text, offset),
                        "{text:?} at {offset}"
                    );
                }
            }
        }
        // Between two queries the cursor stays on the CR of a CRLF.
        let mut locator = Locator::new("a\r\nb");
        assert_eq!(locator.locate(2), SourceLocation::at(2, 1));
        assert_eq!(locator.locate(3), SourceLocation::at(2, 1));
        assert_eq!(locator.locate(4), SourceLocation::at(2, 2));
        assert_eq!(locator.locate(1), SourceLocation::at(1, 2));
    }

    /// `locate` as one scan from the start, the form before `Locator`.
    fn locate_by_scan(text: &str, offset: usize) -> Option<SourceLocation> {
        let before = text.get(..offset)?;
        let before = before.strip_prefix('\u{FEFF}').unwrap_or(before);
        let mut lines = 0_usize;
        let mut line_start = 0;
        let mut after_cr = false;
        for (index, byte) in before.bytes().enumerate() {
            match byte {
                b'\n' if after_cr => line_start = index + 1,
                b'\n' | b'\r' => {
                    lines += 1;
                    line_start = index + 1;
                }
                _ => {}
            }
            after_cr = byte == b'\r';
        }
        let line = u32::try_from(lines + 1).ok()?;
        let column = before.get(line_start..)?.chars().count() + 1;
        SourceLocation::at(line, u32::try_from(column).ok()?)
    }

    #[test]
    fn offset_of_inverts_roxmltree_positions() {
        use roxmltree::TextPos;

        let text = "ab\r\ncé\nd";
        assert_eq!(offset_of(text, TextPos::new(1, 1)), Some(0));
        assert_eq!(
            offset_of(text, TextPos::new(1, 4)),
            Some(3),
            "the CR is a char"
        );
        assert_eq!(offset_of(text, TextPos::new(2, 3)), Some(7), "after the é");
        assert_eq!(offset_of(text, TextPos::new(3, 2)), Some(9), "the end");
        assert_eq!(offset_of(text, TextPos::new(3, 3)), None, "past the end");
        assert_eq!(offset_of(text, TextPos::new(4, 1)), None, "no such row");
        assert_eq!(offset_of(text, TextPos::new(0, 1)), None);
        assert_eq!(offset_of(text, TextPos::new(1, 0)), None);
    }

    #[test]
    fn utf16_and_ucs4_are_recognised_with_and_without_a_bom() {
        let with_bom: [(&[u8], Bom); 6] = [
            (b"\x00\x00\xFE\xFF", Bom::Other("UTF-32 (big-endian)")),
            (b"\xFF\xFE\x00\x00", Bom::Other("UTF-32 (little-endian)")),
            (b"\x00\x00\xFF\xFE", Bom::Other("UCS-4 (byte order 2143)")),
            (b"\xFE\xFF\x00\x00", Bom::Other("UCS-4 (byte order 3412)")),
            (b"\xFE\xFF\x00<", Bom::Utf16(true)),
            (b"\xFF\xFE<\x00", Bom::Utf16(false)),
        ];
        for (input, bom) in with_bom {
            assert_eq!(encoding_by_bom(input), Some(bom), "{input:?}");
        }
        let without_bom: [(&[u8], &str); 6] = [
            (b"\x00\x00\x00<", "UTF-32 (big-endian)"),
            (b"<\x00\x00\x00", "UTF-32 (little-endian)"),
            (b"\x00\x00<\x00", "UCS-4 (byte order 2143)"),
            (b"\x00<\x00\x00", "UCS-4 (byte order 3412)"),
            (b"\x00<\x00?", "UTF-16 (big-endian)"),
            (b"<\x00?\x00", "UTF-16 (little-endian)"),
        ];
        for (input, encoding) in without_bom {
            assert_eq!(encoding_by_bom(input), None, "{input:?}");
            assert_eq!(encoding_by_first_bytes(input), Some(encoding), "{input:?}");
            assert_eq!(
                decode(input).map_err(|error| located(&error)),
                Err((
                    SourceLocation::at(1, 1),
                    format!(
                        "the input is encoded in {encoding}, and only UTF-8, ASCII, ISO-8859-1 and UTF-16 starting with U+FEFF are read (a limit of this version)"
                    ),
                )),
            );
        }
        for input in [&b"<?xml"[..], b"\xEF\xBB\xBF<", b"<\x00", b""] {
            assert_eq!(encoding_by_bom(input), None, "{input:?}");
            assert_eq!(encoding_by_first_bytes(input), None, "{input:?}");
        }
    }

    #[test]
    fn check_declaration_reads_the_encoding_and_refuses_bad_grammar() {
        let declared = |text| {
            check_declaration(text, Encoding::Utf8, None)
                .map(|checked| match checked {
                    Checked::Declared(declared) => declared.map(|d| (d.name, d.offset)),
                    Checked::Latin1 => Some((LATIN_1, 0)),
                })
                .map_err(|error| (error.code() == Code::XML_MALFORMED, error.location()))
        };
        assert_eq!(
            declared("<?xml version=\"1.0\" encoding = 'utf-8'?>"),
            Ok(Some(("utf-8", 32)))
        );
        assert_eq!(
            declared("\u{FEFF}<?xml\nversion='1.0' encoding=\"ascii\"?>"),
            Ok(Some(("ascii", 33)))
        );
        assert_eq!(declared("<?xml version=\"1.0\"?>"), Ok(None));
        assert_eq!(declared("<?xml-model encoding=\"x\"?>"), Ok(None));
        assert_eq!(declared("<r/>"), Ok(None));
        // An unsupported encoding is Unsupported (SACM-XML-002) at its name.
        assert_eq!(
            declared("<?xml version=\"1.0\" encoding=\"x\" junk"),
            Err((false, SourceLocation::at(1, 31)))
        );
        // Missing white space, and an unclosed value or declaration.
        assert_eq!(
            declared("<?xml version=\"1.0\"encoding=\"x\"?>"),
            Err((true, SourceLocation::at(1, 20)))
        );
        assert_eq!(
            declared("<?xml version=\"1.0"),
            Err((true, SourceLocation::at(1, 19)))
        );
        assert_eq!(
            declared("<?xml version=\"1.0\" "),
            Err((true, SourceLocation::at(1, 21)))
        );
        assert_eq!(
            declared("<?xml version \"1.0\"?>"),
            Err((true, SourceLocation::at(1, 15)))
        );
        // A declaration that the valid UTF-8 ends inside: the UTF-8 error.
        for cut in ["<?xml version=\"1", "<?xml version=\"1.0\" st"] {
            assert_eq!(
                check_declaration(cut, Encoding::Utf8, Some(&invalid_utf8(cut)))
                    .map(|checked| matches!(checked, Checked::Declared(Some(_)))),
                Err(invalid_utf8(cut)),
                "{cut:?}"
            );
        }
    }

    /// [`check_unrepaired`] on `text`, run on a worker thread with a 5 s
    /// deadline: a scan that stops advancing fails the test quickly instead
    /// of hanging it (LIB-QA-012).
    fn check(text: &str) -> Result<(), ReadError> {
        use std::sync::mpsc::{RecvTimeoutError, channel};

        let (sender, receiver) = channel();
        let owned = text.to_owned();
        std::thread::spawn(move || {
            // The receiver is gone only after a timeout, which already panics.
            let _ = sender.send(check_unrepaired(&owned));
        });
        match receiver.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => {
                panic!("check_unrepaired did not return within 5 s on {text:?}")
            }
            Err(RecvTimeoutError::Disconnected) => {
                panic!("check_unrepaired panicked on {text:?}")
            }
        }
    }

    /// Where [`check_unrepaired`] refuses `text`, if it does, as [`check`].
    fn at(text: &str) -> Result<(), Option<SourceLocation>> {
        check(text).map_err(|error| error.location())
    }

    #[test]
    fn check_unrepaired_finds_the_first_construct_and_skips_the_rest() {
        assert_eq!(
            at("<r a='&#x41;'>&#65;<![CDATA[&#xD800;]]><!--&#xD800;--></r>"),
            Ok(())
        );
        assert_eq!(at("<r a='&#xD800;'/>"), Err(SourceLocation::at(1, 7)));
        assert_eq!(
            at("<r a=\"x>\" b='&#55296;'/>"),
            Err(SourceLocation::at(1, 14))
        );
        assert_eq!(
            at("<r>a<![CDATA[b]]>&#x110000;</r>"),
            Err(SourceLocation::at(1, 18))
        );
        assert_eq!(at("<r xmlns:p=''/>"), Err(SourceLocation::at(1, 4)));
        assert_eq!(at("<r xmlns=''/>"), Ok(()));
        assert_eq!(at("<?xml version='1.0'?><r/>"), Ok(()));
        assert_eq!(at("<?xml\tversion='1.0'?><r/>"), Ok(()));
        assert_eq!(at("<!--c--><?xMl?><r/>"), Err(SourceLocation::at(1, 9)));
        assert_eq!(at("<?xml-stylesheet a?><r/>"), Ok(()));
        // A comment whose text starts with `>` or `->` ends at the `-->`
        // after its `<!--`, not inside it.
        assert_eq!(
            at("<!--> <x a='--><r a='&#xD800;'/>"),
            Err(SourceLocation::at(1, 22))
        );
        assert_eq!(at("<!-->&#xD800;--><r/>"), Ok(()));
        assert_eq!(at("<!--->&#xD800;--><r/>"), Ok(()));
        assert_eq!(at("<r><!---><?Xml?>--></r>"), Ok(()));
        // A processing instruction's `?>` is looked for after its `<?`.
        assert_eq!(at("<?p?><?Xml?><r/>"), Err(SourceLocation::at(1, 6)));
        // Unclosed constructs stop the scan; they never panic.
        for text in [
            "<r a='",
            "<!--",
            "<?p x",
            "<![CDATA[",
            "<r a",
            "&#",
            "&#x;",
            "<",
        ] {
            assert_eq!(at(text), Ok(()), "{text:?}");
        }
    }

    #[test]
    fn check_unrepaired_checks_processing_instruction_targets() {
        // The target ends where the XML Name ends; `xml` there is reserved,
        // and checked before the white space that must follow a target.
        for pi in [
            "<?xml>?>",
            "<?Xml/?>",
            "<?XML!x?>",
            "<?xml\u{A0}x?>",
            "<?xml=1?>",
            "<?xml\"a\"?>",
            "<?xml'a'?>",
            "<?xml&#xD800;?>",
        ] {
            assert_eq!(
                check(&format!("<r>{pi}</r>")).map_err(|error| error.location()),
                Err(SourceLocation::at(1, 4)),
                "{pi:?}"
            );
        }
        for pi in [
            "<?xml-x?>",
            "<?xml.x?>",
            "<?xml_x?>",
            "<?xml\u{B7}x?>",
            "<?xml\u{E9}?>",
            "<?xml\u{300}?>",
            "<?xml\u{203F}?>",
            "<?xml7?>",
            "<?pi?>",
            "<?pi x?>",
            "<?pi >x?>",
        ] {
            assert_eq!(check(&format!("<r>{pi}</r>")), Ok(()), "{pi:?}");
        }
        // A target followed by neither white space nor `?>` ([16]).
        assert_eq!(at("<r><?pi>x?></r>"), Err(SourceLocation::at(1, 8)));
        assert_eq!(at("<r><?pi/?></r>"), Err(SourceLocation::at(1, 8)));
        assert_eq!(
            check("<r><?Xml>?></r>").map_err(|error| error.to_string()),
            Err("the input is not well-formed XML at line 1, column 4: \
                 a processing instruction has the reserved target \"Xml\""
                .to_owned())
        );
        // A target with a colon, at the `<?`: after the reserved-`xml` check
        // and before [16].
        let reason =
            |text: &str| check(text).map_err(|error| (error.location(), error.to_string()));
        for (pi, target) in [
            ("<?xml:x?>", "xml:x"),
            ("<?a:b x?>", "a:b"),
            ("<?:?>", ":"),
            ("<?p:?>", "p:"),
            ("<?a:b>?>", "a:b"),
        ] {
            assert_eq!(
                reason(&format!("<r>{pi}</r>")),
                Err((
                    SourceLocation::at(1, 4),
                    format!(
                        "the input is not well-formed XML at line 1, column 4: \
                         the processing instruction target {target:?} contains a colon"
                    )
                )),
                "{pi:?}"
            );
        }
    }

    #[test]
    fn check_unrepaired_refuses_names_with_an_empty_prefix() {
        // Names with an empty prefix, at the name.
        for (input, column, name) in [
            ("<:r/>", 2, ":r"),
            ("<:r></r>", 2, ":r"),
            ("<r></:r>", 6, ":r"),
            ("<r><:c/></r>", 5, ":c"),
            ("<r :x=\"1\"/>", 4, ":x"),
            ("<r a='&#xD800;' :x='1'/>", 7, "&#xD800;"),
        ] {
            let result = check(input);
            assert_eq!(
                result.as_ref().map_err(ReadError::location),
                Err(SourceLocation::at(1, column)),
                "{input:?}"
            );
            assert!(
                result.is_err_and(|error| error.to_string().contains(&format!("{name:?}"))),
                "{input:?}"
            );
        }
        assert_eq!(at("<r a:b='1' xmlns:a='urn:a'></r >"), Ok(()));
    }

    #[test]
    fn check_unrepaired_refuses_a_declared_xmlns_prefix() {
        let reason = "the namespace prefix \"xmlns\" is declared, and it cannot be declared";
        // At the attribute name, whatever the value, the quotes or the white
        // space around `=`; an empty value is not reported as undeclaring.
        for (input, column) in [
            ("<r xmlns:xmlns=\"urn:x\"/>", 4),
            ("<r xmlns:xmlns='urn:x'/>", 4),
            ("<r xmlns:xmlns = 'urn:x'/>", 4),
            ("<r a='1'\txmlns:xmlns=''/>", 10),
            ("<r><c xmlns:xmlns='urn:x'/></r>", 7),
            ("<r xmlns:xmlns=\"\"/>", 4),
        ] {
            let result = check(input);
            assert_eq!(
                result.as_ref().map_err(ReadError::location),
                Err(SourceLocation::at(1, column)),
                "{input:?}"
            );
            assert!(
                result.is_err_and(|error| error.to_string().ends_with(reason)),
                "{input:?}"
            );
        }
        // Only the exact name: other letter cases are reserved but not fatal
        // (Namespaces 1.0 §3); and not inside a comment, CDATA section, PI or
        // attribute value.
        for input in [
            "<r xmlns:XMLNS=\"urn:x\"/>",
            "<r xmlns:Xmlns=\"urn:x\"/>",
            "<r xmlns:xmlnsx='urn:x'/>",
            "<r xmlns:xmlfoo='urn:x'/>",
            "<r xmlns:xml='http://www.w3.org/XML/1998/namespace'/>",
            "<r xmlns='urn:x' p:xmlns='1' xmlns:p='urn:p'/>",
            "<!-- <r xmlns:xmlns='x'/> --><r/>",
            "<r><![CDATA[<c xmlns:xmlns='x'/>]]></r>",
            "<r><?pi xmlns:xmlns='x'?></r>",
            "<r a=\" xmlns:xmlns='x'\"/>",
        ] {
            assert_eq!(at(input), Ok(()), "{input:?}");
        }
        // The empty prefix comes first, as `check_name` runs before.
        assert_eq!(
            at("<r :xmlns='x' xmlns:xmlns='y'/>"),
            Err(SourceLocation::at(1, 4))
        );
    }

    #[test]
    fn check_unrepaired_refuses_a_repeated_namespace_declaration() {
        // At the repeat, with `duplicate-attribute` for `xmlns` and
        // `duplicate-prefix` for `xmlns:p`.
        for (input, column, reason) in [
            (
                "<r xmlns='' xmlns=''/>",
                13,
                "an element has the attribute \"xmlns\" more than once",
            ),
            (
                "<r xmlns:p='u' xmlns:p='u'/>",
                16,
                "the namespace prefix \"p\" is declared twice on one element",
            ),
            (
                "<r xmlns:xml='x' xmlns:xml='x'/>",
                18,
                "the namespace prefix \"xml\" is declared twice on one element",
            ),
        ] {
            let result = check(input);
            assert_eq!(
                result.as_ref().map_err(ReadError::location),
                Err(SourceLocation::at(1, column)),
                "{input:?}"
            );
            assert!(
                result.is_err_and(|error| error.to_string().ends_with(reason)),
                "{input:?}"
            );
        }
        // Not a repeat: another letter case, another element, another
        // prefix, or the text inside an attribute value.
        for input in [
            "<r xmlns='' XMLNS=''/>",
            "<r xmlns=''><c xmlns=''/></r>",
            "<r xmlns:p='u' xmlns:q='u'/>",
            "<r a=\" xmlns='' xmlns=''\"/>",
        ] {
            assert_eq!(at(input), Ok(()), "{input:?}");
        }
        // Each declaration is looked up in constant time: a tag of 300,000
        // declarations and a repeat of the first is refused within the
        // deadline of `check`.
        let mut text = String::from("<r");
        text.extend((0..300_000).map(|k| format!(" xmlns:p{k}='u'")));
        text.push_str(" xmlns:p0='u'/>");
        let repeat = text
            .rfind(" xmlns:p0=")
            .and_then(|space| u32::try_from(space + 2).ok())
            .and_then(|column| SourceLocation::at(1, column));
        assert!(repeat.is_some(), "the repeat is in the text");
        assert_eq!(
            at(&text),
            Err(repeat),
            "a repeat after 300,000 declarations"
        );
    }

    #[test]
    fn doctype_offset_skips_the_prolog() {
        assert_eq!(doctype_offset("<!DOCTYPE r>"), Some(0));
        assert_eq!(
            doctype_offset("<?xml version='?>'?>\n<!-- <!DOCTYPE x> --><?pi?> <!DOCTYPE r>"),
            Some(49)
        );
        assert_eq!(doctype_offset("<r/>"), None);
        assert_eq!(doctype_offset("<!-- unclosed"), None);
    }

    #[test]
    fn declaration_end_ignores_quoted_ends() {
        assert_eq!(declaration_end("<?xml a='?>'?>"), Some(14));
        assert_eq!(declaration_end("<?xml a='?>'"), None);
    }

    #[test]
    fn decode_refuses_a_declared_encoding_before_invalid_utf8() {
        // ISO-8859-1 is read since slice 0072 (LIB-COMPAT-016); ISO-8859-2 is
        // not, and 0xE9 is no valid UTF-8.
        let latin2 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-2\"?>\n<r>caf\xE9</r>";
        assert_eq!(
            decode(latin2).map_err(|error| located(&error)),
            Err((
                SourceLocation::at(1, 31),
                "the declared encoding \"ISO-8859-2\" is not supported: without a UTF-16 byte order mark, only UTF-8, ASCII and ISO-8859-1 are read (a limit of this version)"
                    .to_owned(),
            ))
        );
        // The declaration is checked again on the ISO-8859-1 text: the rest of
        // it, `standalone` and `?>`, too.
        let standalone =
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\" standalone=\"maybe\"?><r/>";
        assert_eq!(
            decode(standalone).map_err(|error| (error.template(), error.location())),
            Err(("SACM-XML-001/standalone", SourceLocation::at(1, 55)))
        );
        let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<r>caf\xE9</r>";
        assert_eq!(
            decode(latin1).map_err(|error| located(&error)),
            Ok(Cow::Owned(
                "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<r>caf\u{E9}</r>".to_owned()
            ))
        );
        let ascii = "<?xml version='1.0' encoding='ascii'?>\n<r>é\u{FFFD}</r>";
        assert_eq!(
            decode(ascii.as_bytes()).map_err(|error| error.location()),
            Err(SourceLocation::at(2, 4))
        );
        assert_eq!(
            decode(b"\x00\x00\xFE\xFF").map_err(|error| error.location()),
            Err(SourceLocation::at(1, 1))
        );
    }

    #[test]
    fn decode_reads_utf16_with_a_bom_and_keeps_the_mark() {
        assert_eq!(decode(b"\xFE\xFF\x00<"), Ok(Cow::Borrowed("\u{FEFF}<")));
        assert_eq!(decode(b"\xFF\xFE<\x00"), Ok(Cow::Borrowed("\u{FEFF}<")));
        assert_eq!(
            decode(b"\xFF\xFE\x3D\xD8\x00\xDE"),
            Ok(Cow::Borrowed("\u{FEFF}\u{1F600}"))
        );
        // The UTF-8 path borrows the input.
        assert!(matches!(decode(b"<r/>"), Ok(Cow::Borrowed("<r/>"))));
        // An unpaired surrogate is refused at the surrogate, and an odd byte
        // count after the last whole character.
        let reason = |input: &[u8]| {
            decode(input)
                .map(|_| ())
                .map_err(|error| (error.location(), error.to_string()))
        };
        assert_eq!(
            reason(b"\xFF\xFE<\x00\x00\xD8"),
            Err((
                SourceLocation::at(1, 2),
                "the input is not well-formed XML at line 1, column 2: \
                 the UTF-16 code unit D800 is an unpaired surrogate"
                    .to_owned()
            ))
        );
        assert_eq!(
            reason(b"\xFE\xFF\x00<\x00"),
            Err((
                SourceLocation::at(1, 2),
                "the input is not well-formed XML at line 1, column 2: \
                 the UTF-16 input ends with a byte that is not a whole code unit"
                    .to_owned()
            ))
        );
        let declared = |name: &str| {
            let text = format!("\u{FEFF}<?xml version='1.0' encoding='{name}'?>");
            let input: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            decode(&input).map(|_| ()).map_err(|error| error.code())
        };
        for name in ["UTF-16", "utf-16le", "Utf-16BE"] {
            assert_eq!(declared(name), Ok(()), "{name:?}");
        }
        for name in ["UTF-8", "ascii", "UTF-32"] {
            assert_eq!(declared(name), Err(Code::XML_MALFORMED), "{name:?}");
        }
    }

    #[test]
    fn refusal_locates_and_words_parser_errors() {
        use roxmltree::{Error as E, TextPos};

        let text = "<r>\n<c/>";
        assert_eq!(
            located(&refusal(text, &E::UnclosedRootNode, &Limits::default())),
            (
                SourceLocation::at(2, 5),
                "the root element is not closed".to_owned()
            )
        );
        assert_eq!(
            located(&refusal(
                text,
                &E::UnknownNamespace("p\nq".to_owned(), TextPos::new(2, 2)),
                &Limits::default(),
            )),
            (
                SourceLocation::at(2, 2),
                "the namespace prefix \"p\\nq\" is not declared".to_owned()
            )
        );
        // Parser capacity limits are Unsupported, not Malformed (LIB-SEC-003),
        // the node limit when it is 2^32 or more.
        let unbounded = Limits::default().with_max_nodes(u64::MAX);
        for (limit, template, english) in [
            (
                E::NodesLimitReached,
                "SACM-SEC-002/nodes",
                "the document has more nodes or nesting levels than the parser can hold",
            ),
            (
                E::AttributesLimitReached,
                "SACM-SEC-002/attributes",
                "the document has more attributes than the parser can hold",
            ),
            (
                E::NamespacesLimitReached,
                "SACM-SEC-002/namespaces",
                "the document declares more namespaces than the parser can hold",
            ),
        ] {
            let error = refusal(text, &limit, &unbounded);
            assert!(matches!(error, ReadError::Unsupported { .. }), "{limit:?}");
            // Each limit has its template (LIB-API-017, acceptance 4).
            assert_eq!(
                (error.code(), error.template(), error.arguments()),
                (Code::PARSER_LIMIT, template, &[][..]),
                "LIB-API-017: {limit:?}"
            );
            assert_eq!(
                error.to_string(),
                format!(
                    "the reader cannot represent the input: {english} (a limit of this version)"
                ),
                "LIB-API-017: {limit:?}"
            );
            assert_eq!(
                located(&error),
                (
                    None,
                    format!("{} (a limit of this version)", reason(&limit).render())
                ),
                "{limit:?}"
            );
            assert!(
                reason(&limit)
                    .render()
                    .ends_with("than the parser can hold"),
                "{limit:?}"
            );
        }
        // A leading UTF-8 BOM takes no column, in converted positions too.
        assert_eq!(
            refusal(
                "\u{FEFF}<r></s>",
                &E::UnexpectedCloseTag("r".to_owned(), "s".to_owned(), TextPos::new(1, 5)),
                &Limits::default(),
            )
            .location(),
            SourceLocation::at(1, 4)
        );
        assert_eq!(
            reason(&E::InvalidChar2("'[' or '>'", b'x', TextPos::new(1, 1))).render(),
            "an unexpected character where \"[\" or \">\" was expected"
        );
    }

    /// The parser errors no input reaches: those of a DTD's entity
    /// declarations, which every read refuses first as a DOCTYPE, each with
    /// its template (LIB-API-017, acceptance 4).
    #[test]
    fn the_parser_errors_of_a_dtd_have_their_templates() {
        use roxmltree::{Error as E, TextPos};

        let at = TextPos::new(1, 1);
        let cases = [
            (
                E::UnexpectedEntityCloseTag(at),
                &template::XML_001_ENTITY_CLOSES_ELEMENT,
                "an entity's replacement text closes an element",
            ),
            (
                E::EntityReferenceLoop(at),
                &template::XML_001_ENTITY_LOOP,
                "entity references nest too deeply",
            ),
            (
                E::InvalidAttributeValue(at),
                &template::XML_001_LT_IN_ATTRIBUTE_VALUE,
                "an attribute value holds an unescaped \"<\"",
            ),
            (
                E::InvalidExternalID(at),
                &template::XML_001_INVALID_EXTERNAL_ID,
                "an external identifier is not valid",
            ),
            (
                E::EntityResolver(at, "no".to_owned()),
                &template::XML_001_UNRESOLVED_ENTITY,
                "an external entity could not be resolved",
            ),
        ];
        for (error, template, english) in cases {
            let refused = refusal("<r/>", &error, &Limits::default());
            assert_eq!(
                (refused.code(), refused.template(), refused.arguments()),
                (Code::XML_MALFORMED, template.id, &[][..]),
                "LIB-API-017: {error:?}"
            );
            assert_eq!(
                refused.to_string(),
                format!("the input is not well-formed XML at line 1, column 1: {english}"),
                "LIB-API-017: {error:?}"
            );
        }
        // A DOCTYPE is refused as itself before a reason is asked for.
        assert_eq!(reason(&E::DtdDetected).template(), "SACM-SEC-001/doctype");
    }

    /// The location and the reason of a `Malformed` or `Unsupported`
    /// refusal.
    fn located(error: &ReadError) -> (Option<SourceLocation>, String) {
        match error {
            ReadError::Malformed {
                location, reason, ..
            }
            | ReadError::Unsupported {
                location, reason, ..
            } => (*location, reason.clone()),
            other => panic!("{other:?}"),
        }
    }
}
