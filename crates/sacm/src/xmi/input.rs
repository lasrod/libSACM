//! The input layer of the XMI reader: bytes to a well-formed XML tree, or a
//! refusal (LIB-SEC-001, LIB-SEC-002, LIB-API-009; ADR 0005, ADR 0015).
//!
//! The steps run in this order:
//! 1. UTF-16 or UCS-4 (UTF-32) input is `Unsupported`, recognised by its
//!    byte order mark or, without one, by the first bytes of `<?xml` (XML 1.0
//!    Appendix F).
//! 2. A declared encoding other than UTF-8 or ASCII is `Unsupported`. It is
//!    read from the longest valid UTF-8 prefix, so a Latin-1 file is refused
//!    for its encoding, not as malformed.
//! 3. Under an ASCII declaration, a non-ASCII character is `Malformed`.
//! 4. Invalid UTF-8 is `Malformed`, at the first invalid byte.
//! 5. roxmltree parses with DTDs refused: a DOCTYPE is `Doctype`, and any
//!    other error is `Malformed`.
//!
//! roxmltree stays private to this module: its errors become our own reasons
//! and a [`SourceLocation`].
//!
//! Every location comes from `locate`, which ends lines as XML 1.0 §2.11
//! does: a line feed, a carriage return and line feed, or a lone carriage
//! return each end one line. roxmltree counts only line feeds, so its
//! positions are converted to byte offsets first.

use crate::diagnostic::SourceLocation;

use super::ReadError;

/// The text of `input`, if it is UTF-8 (or ASCII, if declared so) and
/// declares no encoding the reader does not support.
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
    if let Some(declared) = declared_encoding(text) {
        check_declared_encoding(text, &declared)?;
    }
    if utf8_error {
        return Err(ReadError::Malformed {
            location: locate(text, text.len()),
            reason: "a byte sequence is not valid UTF-8".to_owned(),
        });
    }
    Ok(text)
}

/// Parses `text` as XML 1.0 with namespaces, with DTDs refused.
pub(super) fn parse(text: &str) -> Result<roxmltree::Document<'_>, ReadError> {
    // DTDs are disabled by default, so a DOCTYPE is refused before its
    // internal subset is read (ADR 0005); the LIB-SEC-001 tests prove it.
    let options = roxmltree::ParsingOptions::default();
    roxmltree::Document::parse_with_options(text, options).map_err(|error| refusal(text, &error))
}

/// The refusal of a well-formed document, at its root element: this release
/// reads no SACM content (ADR 0015, LIB-API-009).
pub(super) fn unsupported_root(text: &str, document: &roxmltree::Document<'_>) -> ReadError {
    // A parsed document always has a root element.
    match document.root().children().find(roxmltree::Node::is_element) {
        Some(root) => ReadError::Unsupported {
            location: locate(text, root.range().start),
            reason: format!(
                "the root element {:?} and its content are not read in this release",
                root.tag_name().name()
            ),
        },
        None => ReadError::Unsupported {
            location: None,
            reason: "the document is not read in this release".to_owned(),
        },
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

/// Refuses `text` if its declared encoding is not UTF-8 or ASCII, or if it
/// is ASCII and `text` holds a non-ASCII character.
fn check_declared_encoding(text: &str, declared: &Declared<'_>) -> Result<(), ReadError> {
    let name = declared.name;
    if name.eq_ignore_ascii_case("UTF-8") {
        return Ok(());
    }
    if !(name.eq_ignore_ascii_case("US-ASCII") || name.eq_ignore_ascii_case("ASCII")) {
        return Err(ReadError::Unsupported {
            location: locate(text, declared.offset),
            reason: format!(
                "the declared encoding {name:?} is not supported, only UTF-8 and ASCII are read"
            ),
        });
    }
    match text.char_indices().find(|&(_, c)| !c.is_ascii()) {
        Some((offset, c)) => Err(ReadError::Malformed {
            location: locate(text, offset),
            reason: format!(
                "the declared encoding is {name:?}, but the input holds the non-ASCII character U+{:04X}",
                u32::from(c)
            ),
        }),
        None => Ok(()),
    }
}

/// Whether `c` is XML white space (production S).
fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// The text after an optional UTF-8 byte order mark, and its byte offset.
fn after_bom(text: &str) -> (usize, &str) {
    match text.strip_prefix('\u{FEFF}') {
        Some(rest) => ('\u{FEFF}'.len_utf8(), rest),
        None => (0, text),
    }
}

/// The `encoding` of the XML declaration at the start of `text`, if it has
/// one that can be read. A declaration that cannot be read is left to the
/// parser, which refuses it.
fn declared_encoding(text: &str) -> Option<Declared<'_>> {
    let (start, rest) = after_bom(text);
    let mut rest = rest.strip_prefix("<?xml")?;
    let mut offset = start + "<?xml".len();
    loop {
        // Each pseudo-attribute follows white space.
        let trimmed = rest.trim_start_matches(is_xml_space);
        if trimmed.len() == rest.len() {
            return None;
        }
        offset += rest.len() - trimmed.len();
        let name_len = trimmed.find(|c: char| is_xml_space(c) || matches!(c, '=' | '?'))?;
        let (name, after_name) = trimmed.split_at(name_len);
        let after_eq = after_name
            .trim_start_matches(is_xml_space)
            .strip_prefix('=')?
            .trim_start_matches(is_xml_space);
        let quote = after_eq
            .chars()
            .next()
            .filter(|&c| matches!(c, '"' | '\''))?;
        let value_and_rest = after_eq.get(1..)?;
        let value_len = value_and_rest.find(quote)?;
        let value = value_and_rest.get(..value_len)?;
        let value_offset = offset + (trimmed.len() - value_and_rest.len());
        if name == "encoding" {
            return Some(Declared {
                name: value,
                offset: value_offset,
            });
        }
        rest = value_and_rest.get(value_len + 1..)?;
        offset = value_offset + value_len + 1;
    }
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
/// carriage return. Columns count characters. `None` if `offset` is not a
/// character boundary of `text` or the position does not fit in `u32`.
fn locate(text: &str, offset: usize) -> Option<SourceLocation> {
    let before = text.get(..offset)?;
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
    fn declared_encoding_reads_the_pseudo_attribute() {
        let declared = |text| declared_encoding(text).map(|d| (d.name, d.offset));
        assert_eq!(
            declared("<?xml version=\"1.0\" encoding = 'Latin-1'?>"),
            Some(("Latin-1", 32))
        );
        assert_eq!(
            declared("\u{FEFF}<?xml version='?>' encoding=\"x\"?>"),
            Some(("x", 32))
        );
        assert_eq!(declared("<?xml version=\"1.0\"?>"), None);
        assert_eq!(declared("<?xml-model encoding=\"x\"?>"), None);
        assert_eq!(declared("<?xml version=\"1.0\"encoding=\"x\"?>"), None);
        assert_eq!(declared("<?xml version=\"1.0\" encoding=\"x"), None);
        assert_eq!(declared("<r/>"), None);
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
        assert_eq!(
            refusal(text, &E::NodesLimitReached).code(),
            None,
            "a limit is Unsupported"
        );
        assert_eq!(
            reason(&E::InvalidChar2("'[' or '>'", b'x', TextPos::new(1, 1))),
            "an unexpected character where \"[\" or \">\" was expected"
        );
    }
}
