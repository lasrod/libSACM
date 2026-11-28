//! Words diagnostics in another language: looks up each diagnostic's message
//! template, and fills the application's own wording with its arguments.
//!
//! Run it with `cargo run -p sacm --example translate_diagnostics`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, Write as _};

use sacm::diagnostic::{Argument, ArgumentValue, CATALOG_JSON};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions};

/// The application's own wordings, here Swedish, by template id.
///
/// - **The key is stable.** Renaming a template id or an argument is a breaking
///   change. The English text is not promised.
/// - **The placeholders are the catalog's:** `{name}` writes an argument as it
///   is, `{name:?}` writes a text in quotes, and `{name:04X}` a count in hex.
const SWEDISH: [(&str, &str); 2] = [
    (
        "SACM-MULT-001/missing",
        "{class} {id:?} saknar {property} ({property}:{type}[{multiplicity}])",
    ),
    (
        "SACM-REF-001/dangling",
        "{class} {id:?} har {end} {target:?}, som inte finns i dokumentet (ändens typ är {type})",
    ),
];

/// A document with three faults: an attribute that is not SACM 2.3, a Claim
/// with no name, and a source that names no element.
const INPUT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
  <name lang="en" content="Arguments"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1" reviewer="QA">
    <name lang="en" content="The brake stops the vehicle"/>
  </argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c2"/>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2 c9" target="c1">
    <name lang="en" content="Tested, so safe"/>
  </argumentElement>
</sacm:ArgumentPackage>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::stdout().lock();

    // The catalog: every code and every message template of this release, as
    // JSON. Keep the English pattern of each template, by id.
    let catalog: serde_json::Value = serde_json::from_str(CATALOG_JSON)?;
    let templates = catalog
        .get("templates")
        .and_then(serde_json::Value::as_array)
        .ok_or("the catalog has no list of templates")?;
    let mut english = BTreeMap::new();
    for template in templates {
        let id = template.get("id").and_then(serde_json::Value::as_str);
        let pattern = template.get("english").and_then(serde_json::Value::as_str);
        if let (Some(id), Some(pattern)) = (id, pattern) {
            english.insert(id, pattern);
        }
    }
    let swedish = BTreeMap::from(SWEDISH);

    // The check for CI: a wording whose template the catalog no longer has is
    // stale, and a template with no wording falls back to English.
    let stale: Vec<&str> = swedish
        .keys()
        .copied()
        .filter(|id| !english.contains_key(id))
        .collect();
    if !stale.is_empty() {
        return Err(format!("wordings for templates the catalog lacks: {stale:?}").into());
    }
    writeln!(
        out,
        "the catalog has {} templates; {} have a Swedish wording",
        english.len(),
        swedish.len()
    )?;

    // The diagnostics of a tolerant read and of `validate`.
    let options = ReadOptions::default().with_mode(Mode::Tolerant);
    let (document, mut diagnostics) = xmi::read(INPUT.as_bytes(), &options)?.into_parts();
    diagnostics.extend(validation::validate(&document));

    for found in &diagnostics {
        // The template id names the sentence; the arguments fill it.
        let id = found.template();
        let arguments: Vec<String> = found.arguments().iter().map(ToString::to_string).collect();
        writeln!(out)?;
        writeln!(out, "{} {} {id}", found.severity(), found.code())?;
        writeln!(out, "  arguments: {}", arguments.join(", "))?;
        writeln!(out, "  English:   {}", found.message())?;
        let args = found.arguments();
        let text = match (swedish.get(id), english.get(id)) {
            (Some(pattern), _) => render(pattern, args),
            (None, Some(pattern)) => format!("(no wording, so English) {}", render(pattern, args)),
            // A diagnostic with no template has only its message.
            (None, None) => format!("(no template) {}", found.message()),
        };
        writeln!(out, "  Swedish:   {text}")?;
    }
    Ok(())
}

/// The message that `pattern` gives with `arguments`.
///
/// A placeholder that names no argument, or asks for a form its value does
/// not have, is written back as it stands.
fn render(pattern: &str, arguments: &[Argument]) -> String {
    let mut text = String::with_capacity(pattern.len());
    let mut rest = pattern;
    while let Some((before, after)) = rest.split_once('{') {
        text.push_str(before);
        let Some((placeholder, tail)) = after.split_once('}') else {
            text.push('{');
            text.push_str(after);
            return text;
        };
        let (name, form) = placeholder.split_once(':').unwrap_or((placeholder, ""));
        let value = arguments
            .iter()
            .find(|argument| argument.name() == name)
            .map(Argument::value);
        // Writing to a `String` cannot fail.
        let _ = match (value, form) {
            (Some(ArgumentValue::Text(value)), "?") => write!(text, "{value:?}"),
            (Some(ArgumentValue::Count(value)), "04X") => write!(text, "{value:04X}"),
            (Some(value), "") => write!(text, "{value}"),
            _ => write!(text, "{{{placeholder}}}"),
        };
        rest = tail;
    }
    text.push_str(rest);
    text
}
