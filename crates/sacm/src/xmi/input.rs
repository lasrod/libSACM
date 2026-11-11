//! The input layer of the XMI reader: bytes to a well-formed XML tree, or a
//! refusal (LIB-SEC-001, LIB-SEC-002, LIB-SEC-005, LIB-API-009; ADR 0005,
//! ADR 0015, ADR 0016).
//!
//! The steps run in this order:
//! 1. UTF-16 or UCS-4 (UTF-32) input is `Unsupported`, recognised by its
//!    byte order mark or, without one, by the first bytes of `<?xml` (XML 1.0
//!    Appendix F).
//! 2. The XML declaration's `<?xml`, `VersionInfo` and `EncodingDecl` must match
//!    XML 1.0 [23]-[26], [80] and [81], or the input is `Malformed`. The
//!    declaration is read from the longest valid UTF-8 prefix, so a Latin-1
//!    file is refused for its encoding, not as malformed.
//! 3. A declared encoding other than UTF-8 or ASCII is `Unsupported`. For a
//!    supported one, the rest of the declaration (`SDDecl` [32] and `?>`) must
//!    match too.
//! 4. Under an ASCII declaration, a non-ASCII character is `Malformed`.
//! 5. Invalid UTF-8 is `Malformed`, at the first invalid byte.
//! 6. roxmltree parses with DTDs refused: a DOCTYPE is `Doctype`, and any
//!    other error is `Malformed`.
//! 7. What roxmltree accepts but XML 1.0 or Namespaces 1.0 make a fatal error
//!    is `Malformed`, at the earliest offending construct: a character
//!    reference to a surrogate or above U+10FFFF (which roxmltree would turn
//!    into U+FFFD), a prefix undeclared with `xmlns:p=""`, and a processing
//!    instruction whose target is `xml` in any letter case.
//!
//! roxmltree stays private to the `xmi` module: its errors become our own
//! reasons and a [`SourceLocation`].
//!
//! Every location comes from `locate`, which ends lines as XML 1.0 §2.11
//! does: a line feed, a carriage return and line feed, or a lone carriage
//! return each end one line. A leading UTF-8 byte order mark takes no
//! column. roxmltree counts only line feeds, and counts the mark as a
//! character, so its positions are converted to byte offsets first.

use crate::diagnostic::SourceLocation;

use super::ReadError;

/// The text of `input`, if it is UTF-8 (or ASCII, if declared so), its XML
/// declaration is well-formed, and it declares no encoding the reader does
/// not support.
pub(super) fn decode(input: &[u8]) -> Result<&str, ReadError> {
    if let Some(encoding) = encoding_by_bom(input) {
        return Err(ReadError::Unsupported {
            location: SourceLocation::at(1, 1),
            reason: format!(
                "the input starts with a {encoding} byte order mark, and only UTF-8 and ASCII are read"
            ),
        });
    }
    if let Some(encoding) = encoding_by_first_bytes(input) {
        return Err(ReadError::Unsupported {
            location: SourceLocation::at(1, 1),
            reason: format!(
                "the input is encoded in {encoding}, and only UTF-8 and ASCII are read"
            ),
        });
    }
    let (text, utf8_error) = match std::str::from_utf8(input) {
        Ok(text) => (text, false),
        // The bytes up to `valid_up_to` are valid UTF-8, so the fallback is
        // never taken.
        Err(error) => (
            input
                .get(..error.valid_up_to())
                .and_then(|valid| std::str::from_utf8(valid).ok())
                .unwrap_or_default(),
            true,
        ),
    };
    if let Some(declared) = check_declaration(text, utf8_error)? {
        check_ascii(text, &declared)?;
    }
    if utf8_error {
        return Err(invalid_utf8(text));
    }
    Ok(text)
}

/// Parses `text` as XML 1.0 with namespaces, with DTDs refused, and refuses
/// what roxmltree accepts but XML 1.0 or Namespaces 1.0 do not.
pub(super) fn parse(text: &str) -> Result<roxmltree::Document<'_>, ReadError> {
    // DTDs are disabled by default, so a DOCTYPE is refused before its
    // internal subset is read (ADR 0005); the LIB-SEC-001 tests prove it.
    let options = roxmltree::ParsingOptions::default();
    let document = roxmltree::Document::parse_with_options(text, options)
        .map_err(|error| refusal(text, &error))?;
    check_unrepaired(text)?;
    Ok(document)
}

/// The refusal of invalid UTF-8 after `text`, the longest valid prefix.
fn invalid_utf8(text: &str) -> ReadError {
    ReadError::Malformed {
        location: locate(text, text.len()),
        reason: "a byte sequence is not valid UTF-8".to_owned(),
    }
}

/// A `Malformed` refusal at byte `offset` of `text`.
fn malformed(text: &str, offset: usize, reason: String) -> ReadError {
    ReadError::Malformed {
        location: locate(text, offset),
        reason,
    }
}

/// The encoding named by a UTF-16 or UCS-4 byte order mark at the start of
/// `input` (XML 1.0 Appendix F).
fn encoding_by_bom(input: &[u8]) -> Option<&'static str> {
    // The four-byte marks first: two of them start with a UTF-16 mark.
    match input {
        [0x00, 0x00, 0xFE, 0xFF, ..] => Some("UTF-32 (big-endian)"),
        [0xFF, 0xFE, 0x00, 0x00, ..] => Some("UTF-32 (little-endian)"),
        [0x00, 0x00, 0xFF, 0xFE, ..] => Some("UCS-4 (byte order 2143)"),
        [0xFE, 0xFF, 0x00, 0x00, ..] => Some("UCS-4 (byte order 3412)"),
        [0xFE, 0xFF, ..] => Some("UTF-16 (big-endian)"),
        [0xFF, 0xFE, ..] => Some("UTF-16 (little-endian)"),
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

/// Refuses `declared` if it is not UTF-8 or ASCII, in any letter case.
fn check_supported(text: &str, declared: &Declared<'_>) -> Result<(), ReadError> {
    let name = declared.name;
    if ["UTF-8", "US-ASCII", "ASCII"]
        .iter()
        .any(|supported| name.eq_ignore_ascii_case(supported))
    {
        return Ok(());
    }
    Err(ReadError::Unsupported {
        location: locate(text, declared.offset),
        reason: format!(
            "the declared encoding {name:?} is not supported, only UTF-8 and ASCII are read"
        ),
    })
}

/// Refuses `text` if `declared`, a supported encoding, is ASCII and `text`
/// holds a non-ASCII character.
fn check_ascii(text: &str, declared: &Declared<'_>) -> Result<(), ReadError> {
    let name = declared.name;
    if name.eq_ignore_ascii_case("UTF-8") {
        return Ok(());
    }
    match text.char_indices().find(|&(_, c)| !c.is_ascii()) {
        Some((offset, c)) => Err(malformed(
            text,
            offset,
            format!(
                "the declared encoding is {name:?}, but the input holds the non-ASCII character U+{:04X}",
                u32::from(c)
            ),
        )),
        None => Ok(()),
    }
}

/// Whether `c` is XML white space (production S).
pub(super) fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// The text after an optional UTF-8 byte order mark, and its byte offset.
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
/// encoding is supported, and only then `SDDecl` and `?>`. `truncated` says
/// that `text` is the valid UTF-8 prefix of a longer input: a declaration
/// that `text` ends inside is then refused for the invalid UTF-8.
fn check_declaration(text: &str, truncated: bool) -> Result<Option<Declared<'_>>, ReadError> {
    let Some(start) = declaration_start(text) else {
        return Ok(None);
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
            format!("the XML version {version:?} is not \"1.\" followed by digits"),
        ));
    }
    // EncodingDecl [80], then the supported-encoding check.
    let mut spaced = cursor.skip_space();
    let mut encoding = None;
    if spaced && cursor.peek_name() == "encoding" {
        cursor.name();
        let (offset, name) = cursor.value()?;
        if !is_encoding_name(name) {
            return Err(malformed(
                text,
                offset,
                format!("the encoding name {name:?} is not a valid encoding name"),
            ));
        }
        let declared = Declared { name, offset };
        check_supported(text, &declared)?;
        encoding = Some(declared);
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
                format!("the standalone value {standalone:?} is not \"yes\" or \"no\""),
            ));
        }
        spaced = cursor.skip_space();
    }
    if cursor.rest().starts_with("?>") {
        return Ok(encoding);
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
struct Cursor<'a> {
    /// The whole text.
    text: &'a str,
    /// The byte offset of the next character.
    offset: usize,
    /// Whether `text` is the valid UTF-8 prefix of a longer input.
    truncated: bool,
}

impl<'a> Cursor<'a> {
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
    /// before it. A name that invalid UTF-8 cuts short is refused for the
    /// invalid UTF-8.
    fn misplaced(&self, offset: usize, name: &str, spaced: bool, expected: &str) -> ReadError {
        let reason = if name.is_empty() {
            return self.unexpected(expected);
        } else if self.truncated && offset + name.len() == self.text.len() {
            return self.ended();
        } else if !PSEUDO_ATTRIBUTES.contains(&name) {
            format!("the XML declaration has the unknown pseudo-attribute {name:?}")
        } else if spaced {
            format!("the pseudo-attribute {name:?} is out of order in the XML declaration")
        } else {
            format!(
                "the pseudo-attribute {name:?} does not follow white space in the XML declaration"
            )
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
            format!("an unexpected character in the XML declaration where {expected} was expected"),
        )
    }

    /// The refusal of a declaration that the text ends inside.
    fn ended(&self) -> ReadError {
        if self.truncated {
            invalid_utf8(self.text)
        } else {
            malformed(
                self.text,
                self.text.len(),
                "the input ends inside the XML declaration".to_owned(),
            )
        }
    }
}

/// Refuses the first construct in `text` that XML 1.0 or Namespaces 1.0
/// make a fatal error but roxmltree lets through (LIB-SEC-005): a character
/// reference to a surrogate or above U+10FFFF, a prefix undeclared with
/// `xmlns:p=""`, or a processing instruction targeted `xml` in any letter
/// case.
///
/// It scans the text once, in document order, so the earliest construct is
/// refused. It runs only on text roxmltree has accepted, where comments,
/// processing instructions, CDATA sections and tags are closed and attribute
/// values are quoted; should one not be, the scan stops without a refusal.
/// It scans the raw text rather than the tree, because roxmltree merges
/// adjacent text and CDATA into one node whose range covers only the first.
/// The XML declaration has been checked by [`decode`].
fn check_unrepaired(text: &str) -> Result<(), ReadError> {
    let declaration = declaration_start(text);
    let mut offset = 0;
    while let Some(rest) = text.get(offset..).filter(|rest| !rest.is_empty()) {
        let length = if rest.starts_with("<!--") {
            rest.find("-->").map(|end| end + "-->".len())
        } else if rest.starts_with("<![CDATA[") {
            rest.find("]]>").map(|end| end + "]]>".len())
        } else if let Some(body) = rest.strip_prefix("<?") {
            let target_length = body
                .find(|c: char| is_xml_space(c) || c == '?')
                .unwrap_or(body.len());
            let target = body.get(..target_length).unwrap_or_default();
            if target.eq_ignore_ascii_case("xml") && Some(offset) != declaration {
                return Err(malformed(
                    text,
                    offset,
                    format!("a processing instruction has the reserved target {target:?}"),
                ));
            }
            rest.find("?>").map(|end| end + "?>".len())
        } else if rest.starts_with("</") || rest.starts_with("<!") {
            rest.find('>').map(|end| end + 1)
        } else if rest.starts_with('<') {
            start_tag(text, offset)?
        } else {
            let length = rest.find('<').unwrap_or(rest.len());
            check_references(text, offset, length)?;
            Some(length)
        };
        match length {
            Some(length) if length > 0 => offset += length,
            _ => break,
        }
    }
    Ok(())
}

/// Checks the start tag at byte `offset` of `text`: its namespace
/// declarations, and the character references in its attribute values.
/// Returns its length, or `None` if it does not end.
fn start_tag(text: &str, offset: usize) -> Result<Option<usize>, ReadError> {
    let tag = text.get(offset..).unwrap_or_default();
    // Past the `<` and the element name.
    let mut index = tag
        .find(|c: char| is_xml_space(c) || matches!(c, '/' | '>'))
        .unwrap_or(tag.len());
    loop {
        let Some(rest) = tag.get(index..) else {
            return Ok(None);
        };
        let trimmed = rest.trim_start_matches(is_xml_space);
        index += rest.len() - trimmed.len();
        if trimmed.starts_with('>') {
            return Ok(Some(index + 1));
        }
        if trimmed.starts_with("/>") {
            return Ok(Some(index + 2));
        }
        // An attribute: its name, `Eq` and a quoted value.
        let Some(name_length) = trimmed.find(|c: char| is_xml_space(c) || c == '=') else {
            return Ok(None);
        };
        let name = trimmed.get(..name_length).unwrap_or_default();
        let Some(open) = trimmed.find(['"', '\'']) else {
            return Ok(None);
        };
        let quote = trimmed.get(open..).and_then(|q| q.chars().next());
        let value_start = index + open + 1;
        let Some(value_length) = quote.and_then(|q| tag.get(value_start..)?.find(q)) else {
            return Ok(None);
        };
        if let Some(prefix) = name.strip_prefix("xmlns:")
            && value_length == 0
        {
            return Err(malformed(
                text,
                offset + index,
                format!(
                    "the namespace prefix {prefix:?} is declared with an empty value, and a prefix cannot be undeclared"
                ),
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
            Some(value) if value <= 0x10_FFFF => "a surrogate",
            _ => "a code point above U+10FFFF",
        };
        let reference = after
            .get(..after.len() - digits.len() + end + 1)
            .map(|body| format!("&#{body}"))
            .unwrap_or_default();
        return Err(malformed(
            text,
            offset + index,
            format!(
                "the character reference {reference:?} refers to {what}, which is not a character"
            ),
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
/// carriage return. Columns count characters; a leading UTF-8 byte order
/// mark is an encoding signature, not text, and takes no column. `None` if
/// `offset` is not a character boundary of `text` or the position does not
/// fit in `u32`.
pub(super) fn locate(text: &str, offset: usize) -> Option<SourceLocation> {
    let before = text.get(..offset)?;
    let before = before.strip_prefix('\u{FEFF}').unwrap_or(before);
    let mut lines = 0_usize;
    let mut line_start = 0;
    let mut after_cr = false;
    for (index, byte) in before.bytes().enumerate() {
        match byte {
            // The line feed of a CRLF: the CR already ended the line.
            b'\n' if after_cr => line_start = index + 1,
            b'\n' | b'\r' => {
                lines = lines.checked_add(1)?;
                line_start = index + 1;
            }
            _ => {}
        }
        after_cr = byte == b'\r';
    }
    let line = u32::try_from(lines.checked_add(1)?).ok()?;
    let column = before.get(line_start..)?.chars().count().checked_add(1)?;
    SourceLocation::at(line, u32::try_from(column).ok()?)
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
/// end of the input.
fn refusal(text: &str, error: &roxmltree::Error) -> ReadError {
    use roxmltree::Error as E;

    match error {
        E::DtdDetected => ReadError::Doctype {
            location: doctype_offset(text).and_then(|offset| locate(text, offset)),
        },
        E::NodesLimitReached | E::AttributesLimitReached | E::NamespacesLimitReached => {
            ReadError::Unsupported {
                location: None,
                reason: reason(error),
            }
        }
        E::NoRootNode | E::UnclosedRootNode | E::UnexpectedEndOfStream => ReadError::Malformed {
            location: locate(text, text.len()),
            reason: reason(error),
        },
        _ => ReadError::Malformed {
            location: offset_of(text, error.pos()).and_then(|offset| locate(text, offset)),
            reason: reason(error),
        },
    }
}

/// Our own reason for a roxmltree error. It states no position, and it
/// quotes names from the input with `{:?}`, so it stays on one line.
/// roxmltree's `Display` does neither.
fn reason(error: &roxmltree::Error) -> String {
    use roxmltree::Error as E;

    match error {
        E::DtdDetected => "the input has a DOCTYPE".to_owned(),
        E::NodesLimitReached => {
            "the document has more nodes or nesting levels than the parser can hold".to_owned()
        }
        E::AttributesLimitReached => {
            "the document has more attributes than the parser can hold".to_owned()
        }
        E::NamespacesLimitReached => {
            "the document declares more namespaces than the parser can hold".to_owned()
        }
        E::NoRootNode => "the document has no root element".to_owned(),
        E::UnclosedRootNode => "the root element is not closed".to_owned(),
        E::UnexpectedEndOfStream => "the input ends before the document is complete".to_owned(),
        E::InvalidXmlPrefixUri(_) => {
            "the \"xml\" prefix is bound to a namespace other than the XML namespace".to_owned()
        }
        E::UnexpectedXmlUri(_) => {
            "the XML namespace is bound to a prefix other than \"xml\"".to_owned()
        }
        E::UnexpectedXmlnsUri(_) => {
            "the \"xmlns\" namespace is declared, which is not allowed".to_owned()
        }
        E::InvalidElementNamePrefix(_) => "an element name has the prefix \"xmlns\"".to_owned(),
        E::DuplicatedNamespace(prefix, _) => {
            format!("the namespace prefix {prefix:?} is declared twice on one element")
        }
        E::UnknownNamespace(prefix, _) => {
            format!("the namespace prefix {prefix:?} is not declared")
        }
        E::UnexpectedCloseTag(expected, actual, _) => {
            format!("the end tag {actual:?} does not match the start tag {expected:?}")
        }
        E::UnexpectedEntityCloseTag(_) => {
            "an entity's replacement text closes an element".to_owned()
        }
        E::UnknownEntityReference(name, _) => format!("the entity {name:?} is not declared"),
        E::MalformedEntityReference(_) => {
            "a \"&\" does not start an entity or character reference".to_owned()
        }
        E::EntityReferenceLoop(_) => "entity references nest too deeply".to_owned(),
        E::InvalidAttributeValue(_) => "an attribute value holds an unescaped \"<\"".to_owned(),
        E::DuplicatedAttribute(name, _) => {
            format!("an element has the attribute {name:?} more than once")
        }
        E::UnexpectedDeclaration(_) => {
            "an XML declaration is not at the start of the document".to_owned()
        }
        E::InvalidName(_) => "a name is not valid".to_owned(),
        E::NonXmlChar(c, _) => {
            format!(
                "the character U+{:04X} is not allowed in XML",
                u32::from(*c)
            )
        }
        E::InvalidChar(expected, _, _) => format!(
            "an unexpected character where {:?} was expected",
            char::from(*expected).to_string()
        ),
        // roxmltree's descriptions are fixed English phrases that
        // single-quote characters, as in "'>'".
        E::InvalidChar2(expected, _, _) => format!(
            "an unexpected character where {} was expected",
            expected
                .replace("a whitespace", "white space")
                .replace('\'', "\"")
        ),
        E::InvalidString(expected, _) => format!("{expected:?} was expected"),
        E::InvalidExternalID(_) => "an external identifier is not valid".to_owned(),
        E::EntityResolver(..) => "an external entity could not be resolved".to_owned(),
        E::InvalidComment(_) => "a comment holds \"--\" or ends with \"-\"".to_owned(),
        E::InvalidCharacterData(_) => "character data holds \"]]>\"".to_owned(),
        E::UnknownToken(_) => "the input has markup or text that is not allowed here".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let with_bom: [(&[u8], &str); 6] = [
            (b"\x00\x00\xFE\xFF", "UTF-32 (big-endian)"),
            (b"\xFF\xFE\x00\x00", "UTF-32 (little-endian)"),
            (b"\x00\x00\xFF\xFE", "UCS-4 (byte order 2143)"),
            (b"\xFE\xFF\x00\x00", "UCS-4 (byte order 3412)"),
            (b"\xFE\xFF\x00<", "UTF-16 (big-endian)"),
            (b"\xFF\xFE<\x00", "UTF-16 (little-endian)"),
        ];
        for (input, encoding) in with_bom {
            assert_eq!(encoding_by_bom(input), Some(encoding), "{input:?}");
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
                decode(input),
                Err(ReadError::Unsupported {
                    location: SourceLocation::at(1, 1),
                    reason: format!(
                        "the input is encoded in {encoding}, and only UTF-8 and ASCII are read"
                    ),
                }),
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
            check_declaration(text, false)
                .map(|declared| declared.map(|d| (d.name, d.offset)))
                .map_err(|error| (error.code().is_some(), error.location()))
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
        // An unsupported encoding is Unsupported (no code) at its name.
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
                check_declaration(cut, true).map(|declared| declared.is_some()),
                Err(invalid_utf8(cut)),
                "{cut:?}"
            );
        }
    }

    #[test]
    fn check_unrepaired_finds_the_first_construct_and_skips_the_rest() {
        let at = |text| check_unrepaired(text).map_err(|error| error.location());
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
        // Unclosed constructs stop the scan; they never panic.
        for text in [
            "<r a='",
            "<!--",
            "<?p",
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
        let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<r>caf\xE9</r>";
        assert_eq!(
            decode(latin1),
            Err(ReadError::Unsupported {
                location: SourceLocation::at(1, 31),
                reason: "the declared encoding \"ISO-8859-1\" is not supported, only UTF-8 and ASCII are read"
                    .to_owned(),
            })
        );
        let ascii = "<?xml version='1.0' encoding='ascii'?>\n<r>é\u{FFFD}</r>";
        assert_eq!(
            decode(ascii.as_bytes()).map_err(|error| error.location()),
            Err(SourceLocation::at(2, 4))
        );
        assert_eq!(
            decode(b"\xFE\xFF\x00<").map_err(|error| error.location()),
            Err(SourceLocation::at(1, 1))
        );
    }

    #[test]
    fn refusal_locates_and_words_parser_errors() {
        use roxmltree::{Error as E, TextPos};

        let text = "<r>\n<c/>";
        assert_eq!(
            refusal(text, &E::UnclosedRootNode),
            ReadError::Malformed {
                location: SourceLocation::at(2, 5),
                reason: "the root element is not closed".to_owned(),
            }
        );
        assert_eq!(
            refusal(
                text,
                &E::UnknownNamespace("p\nq".to_owned(), TextPos::new(2, 2))
            ),
            ReadError::Malformed {
                location: SourceLocation::at(2, 2),
                reason: "the namespace prefix \"p\\nq\" is not declared".to_owned(),
            }
        );
        // Parser capacity limits are Unsupported, not Malformed (LIB-SEC-003).
        for limit in [
            E::NodesLimitReached,
            E::AttributesLimitReached,
            E::NamespacesLimitReached,
        ] {
            assert_eq!(
                refusal(text, &limit),
                ReadError::Unsupported {
                    location: None,
                    reason: reason(&limit),
                },
                "{limit:?}"
            );
            assert!(
                reason(&limit).ends_with("than the parser can hold"),
                "{limit:?}"
            );
        }
        // A leading UTF-8 BOM takes no column, in converted positions too.
        assert_eq!(
            refusal(
                "\u{FEFF}<r></s>",
                &E::UnexpectedCloseTag("r".to_owned(), "s".to_owned(), TextPos::new(1, 5))
            )
            .location(),
            SourceLocation::at(1, 4)
        );
        assert_eq!(
            reason(&E::InvalidChar2("'[' or '>'", b'x', TextPos::new(1, 1))),
            "an unexpected character where \"[\" or \">\" was expected"
        );
    }
}
