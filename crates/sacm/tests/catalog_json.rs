//! Tests for the machine-readable catalog, `sacm::diagnostic::CATALOG_JSON`
//! and the file `src/diagnostic/catalog.json` (LIB-DIAG-006; ADR 0011
//! amendment 5).
//!
//! - `CATALOG_JSON` is the shipped file byte for byte: `format` 1, then the
//!   specifications, the codes with their severities and the templates with
//!   their code, carrier, arguments and English, in that order.
//! - With the `serde` feature: every listed code, severity and
//!   specification reads as the crate's; every template reads back as a
//!   record of its carrier, built from test values (text `"t"`, count `1`,
//!   prose `"p"`), and is refused with one argument of the other JSON type;
//!   and every diagnostic and refusal the fixtures give matches its entry:
//!   code, carrier, arguments, the English rendered from its arguments, and
//!   a severity the entry lists (`"error"` for a refusal).
//!
//! The English is rendered here by the placeholder rules of
//! `docs/sacm/diagnostics.md`: `{n}` writes a text or prose as it is and a
//! count in decimal, `{n:?}` a text quoted as Rust's `Debug` quotes it, and
//! `{n:04X}` a count as at least four uppercase hex digits.
//!
//! Not checked here:
//! - That `cargo xtask catalog` writes the file from template.rs,
//!   diagnostic.rs and diagnostics.md: `xtask/tests/catalog.rs`.

use sacm::diagnostic::CATALOG_JSON;
use serde_json::Value;

const ROW: &str = "LIB-DIAG-006";

/// The shipped file.
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/diagnostic/catalog.json");

/// The start of the catalog, as the design gives it.
const HEAD: &str = r#"{
  "format": 1,
  "specifications": [
    "SACM 2.3",
    "XMI 2.5.1"
  ],
  "codes": [
    {
      "code": "SACM-ABS-001",
      "severities": [
        "error",
        "warning"
      ]
    },
"#;

/// The entry of the value template of slice 0082 (LIB-API-013), as the
/// design gives it.
const VALUE_POSITION_ENTRY: &str = r#"{
  "id": "SACM-XML-003/unrepresentable-character-in-value",
  "code": "SACM-XML-003",
  "carrier": "WriteError::UnrepresentableCharacter",
  "arguments": [
    { "name": "element", "kind": "text" },
    { "name": "attribute", "kind": "prose" },
    { "name": "position", "kind": "count" },
    { "name": "code_point", "kind": "count" }
  ],
  "english": "the element {element:?} cannot be written: its {attribute} at index {position} holds U+{code_point:04X}, which XML 1.0 cannot represent"
}"#;

/// `CATALOG_JSON`, parsed.
fn catalog() -> Value {
    match serde_json::from_str(CATALOG_JSON) {
        Ok(value) => value,
        Err(error) => panic!("{ROW}: CATALOG_JSON is not JSON: {error}"),
    }
}

/// The array `key` of the catalog.
fn list<'v>(catalog: &'v Value, key: &str) -> &'v [Value] {
    match catalog.get(key).and_then(Value::as_array) {
        Some(items) => items,
        None => panic!("{ROW}: the catalog has no array {key:?}"),
    }
}

/// The template entry `id` of the catalog.
fn template_entry<'v>(catalog: &'v Value, id: &str) -> &'v Value {
    match list(catalog, "templates")
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
    {
        Some(entry) => entry,
        None => panic!("{ROW}: the catalog has no template {id:?}"),
    }
}

// ---------------------------------------------------------------------------
// The shipped file
// ---------------------------------------------------------------------------

#[test]
fn lib_diag_006_catalog_json_is_the_shipped_file() {
    let shipped = std::fs::read_to_string(SHIPPED)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {SHIPPED}: {error}"));
    assert_eq!(
        CATALOG_JSON, shipped,
        "{ROW}: CATALOG_JSON is not src/diagnostic/catalog.json"
    );
    let catalog = catalog();
    let keys: Vec<&str> = catalog
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        ["codes", "format", "specifications", "templates"],
        "{ROW}: the top-level keys"
    );
    let positions: Vec<Option<usize>> = ["format", "specifications", "codes", "templates"]
        .iter()
        .map(|key| CATALOG_JSON.find(&format!("\n  \"{key}\": ")))
        .collect();
    assert!(
        positions.iter().all(Option::is_some) && positions.windows(2).all(|w| w[0] < w[1]),
        "{ROW}: the top-level keys are not format, specifications, codes, templates in \
         that order: {positions:?}"
    );
    assert_eq!(
        catalog.get("format").and_then(Value::as_u64),
        Some(1),
        "{ROW}: format"
    );
    assert!(
        CATALOG_JSON.starts_with(HEAD),
        "{ROW}: the catalog does not start with the designed head"
    );
    assert!(
        CATALOG_JSON.ends_with("}\n") && !CATALOG_JSON.ends_with("\n\n"),
        "{ROW}: the catalog does not end with one line feed after its object"
    );
}

#[test]
fn lib_diag_006_the_catalog_holds_the_value_position_template() {
    let expected: Value = match serde_json::from_str(VALUE_POSITION_ENTRY) {
        Ok(value) => value,
        Err(error) => panic!("{ROW}: setup: the excerpt is not JSON: {error}"),
    };
    let catalog = catalog();
    assert_eq!(
        template_entry(&catalog, "SACM-XML-003/unrepresentable-character-in-value"),
        &expected,
        "{ROW}: the entry of the value template"
    );
}

#[cfg(feature = "serde")]
mod with_serde {
    //! The `serde` checks, which read the catalog's names as the crate's
    //! types.

    use std::collections::BTreeMap;

    use sacm::diagnostic::{Argument, ArgumentValue, Code, Diagnostic, Severity, Specification};
    use sacm::edit::{
        self, CreateArgumentPackage, CreateAssertedInference, CreateClaim, CreateNote,
        DeleteElement, Operation, ReorderElements, SetContent, SetName,
    };
    use sacm::model::{Document, LangString, MultiLangString};
    use sacm::validation;
    use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteError, WriteOptions};
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use serde_json::Value;

    use super::{ROW, catalog, list, template_entry};

    /// The string `key` of `entry`.
    fn string<'v>(entry: &'v Value, key: &str) -> &'v str {
        match entry.get(key).and_then(Value::as_str) {
            Some(text) => text,
            None => panic!("{ROW}: {entry} has no string {key:?}"),
        }
    }

    /// The `(name, kind)` of each argument of `entry`, in order.
    fn entry_arguments(entry: &Value) -> Vec<(String, String)> {
        list(entry, "arguments")
            .iter()
            .map(|argument| {
                (
                    string(argument, "name").to_owned(),
                    string(argument, "kind").to_owned(),
                )
            })
            .collect()
    }

    /// The kind of `value` as the catalog names it.
    fn kind_of(value: &ArgumentValue) -> &'static str {
        match value {
            ArgumentValue::Text(_) => "text",
            ArgumentValue::Count(_) => "count",
            ArgumentValue::Prose(_) => "prose",
            other => panic!("{ROW}: an argument kind this test does not know: {other:?}"),
        }
    }

    /// `pattern` with each placeholder written from the argument it names, by
    /// the rules of the module docs; a placeholder that names no argument, or
    /// one of the other kind, is written back as written.
    fn render(pattern: &str, arguments: &[(&str, &ArgumentValue)]) -> String {
        let mut out = String::new();
        let mut rest = pattern;
        while let Some((before, after)) = rest.split_once('{') {
            out.push_str(before);
            let Some((inner, tail)) = after.split_once('}') else {
                out.push('{');
                out.push_str(after);
                return out;
            };
            let (name, format) = inner.split_once(':').unwrap_or((inner, ""));
            let value = arguments
                .iter()
                .find(|(argument, _)| *argument == name)
                .map(|(_, value)| *value);
            let written = match (value, format) {
                (Some(ArgumentValue::Text(text) | ArgumentValue::Prose(text)), "") => text.clone(),
                (Some(ArgumentValue::Text(text)), "?") => format!("{text:?}"),
                (Some(ArgumentValue::Count(count)), "") => count.to_string(),
                (Some(ArgumentValue::Count(count)), "04X") => format!("{count:04X}"),
                _ => format!("{{{inner}}}"),
            };
            out.push_str(&written);
            rest = tail;
        }
        out.push_str(rest);
        out
    }

    /// The directory of the golden JSON files.
    const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

    /// The test data.
    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

    /// The severities of each code of the catalog.
    fn severities(catalog: &Value) -> BTreeMap<String, Vec<String>> {
        list(catalog, "codes")
            .iter()
            .map(|entry| {
                let listed = list(entry, "severities")
                    .iter()
                    .map(|severity| match severity.as_str() {
                        Some(name) => name.to_owned(),
                        None => panic!("{ROW}: {entry}: a severity that is not a string"),
                    })
                    .collect();
                (string(entry, "code").to_owned(), listed)
            })
            .collect()
    }

    fn to_value<T: Serialize>(value: &T) -> Value {
        match serde_json::to_value(value) {
            Ok(value) => value,
            Err(error) => panic!("{ROW}: a value does not serialize: {error}"),
        }
    }

    fn from_value<T: DeserializeOwned>(what: &str, value: Value) -> T {
        match serde_json::from_value(value) {
            Ok(read) => read,
            Err(error) => panic!("{ROW}: {what} does not read: {error}"),
        }
    }

    #[test]
    fn lib_diag_006_every_listed_code_and_specification_is_the_crate_s() {
        let catalog = catalog();
        let codes = list(&catalog, "codes");
        assert_eq!(codes.len(), 46, "{ROW}: the number of codes");
        for entry in codes {
            let text = string(entry, "code");
            let code: Code = from_value(text, Value::String(text.to_owned()));
            assert_eq!(code.as_str(), text, "{ROW}: {text} reads as another code");
            assert_eq!(
                to_value(&code),
                Value::String(text.to_owned()),
                "{ROW}: {text} does not write back as itself"
            );
            for severity in list(entry, "severities") {
                let read: Severity = from_value(text, severity.clone());
                assert_eq!(
                    to_value(&read),
                    *severity,
                    "{ROW}: {text}: the severity {severity} does not write back as itself"
                );
            }
        }
        let names: Vec<&str> = list(&catalog, "specifications")
            .iter()
            .map(|name| name.as_str().unwrap_or_default())
            .collect();
        assert_eq!(
            names,
            ["SACM 2.3", "XMI 2.5.1"],
            "{ROW}: the specifications"
        );
        assert_eq!(
            names,
            [
                Specification::Sacm23.as_str(),
                Specification::Xmi251.as_str()
            ],
            "{ROW}: the specifications are not the crate's"
        );
        for (name, specification) in names
            .iter()
            .zip([Specification::Sacm23, Specification::Xmi251])
        {
            let read: Specification = from_value(name, Value::String((*name).to_owned()));
            assert_eq!(
                read, specification,
                "{ROW}: {name} reads as another specification"
            );
        }
    }

    /// The test value of an argument of `kind`.
    fn test_value(kind: &str) -> Value {
        match kind {
            "text" => Value::String("t".to_owned()),
            "count" => Value::from(1_u64),
            "prose" => Value::String("p".to_owned()),
            other => panic!("{ROW}: an argument kind the catalog does not define: {other:?}"),
        }
    }

    /// The test value of an argument of `kind`, as the crate holds it.
    fn test_argument(kind: &str) -> ArgumentValue {
        match kind {
            "text" => ArgumentValue::Text("t".to_owned()),
            "count" => ArgumentValue::Count(1),
            "prose" => ArgumentValue::Prose("p".to_owned()),
            other => panic!("{ROW}: an argument kind the catalog does not define: {other:?}"),
        }
    }

    /// The first object of the golden file `name` whose `type` is `variant`.
    fn golden_variant(name: &str, variant: &str) -> serde_json::Map<String, Value> {
        let path = std::path::Path::new(GOLDEN).join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
        let values: Vec<Value> = match serde_json::from_str(&text) {
            Ok(values) => values,
            Err(error) => panic!("{ROW}: {name} is not a JSON array: {error}"),
        };
        values
            .into_iter()
            .filter_map(|value| value.as_object().cloned())
            .find(|object| object.get("type").and_then(Value::as_str) == Some(variant))
            .unwrap_or_else(|| panic!("{ROW}: setup: {name} has no {variant}"))
    }

    /// A record of `entry` built from the test values, and the same record
    /// with its first argument of the other JSON type, if it has one.
    fn records(
        entry: &Value,
        severities: &BTreeMap<String, Vec<String>>,
    ) -> (Value, Option<Value>) {
        let id = string(entry, "id");
        let code = string(entry, "code");
        let carrier = string(entry, "carrier");
        let arguments: serde_json::Map<String, Value> = entry_arguments(entry)
            .into_iter()
            .map(|(name, kind)| (name, test_value(&kind)))
            .collect();
        let mut record = if carrier == "Diagnostic" {
            let first = severities
                .get(code)
                .and_then(|listed| listed.first())
                .unwrap_or_else(|| panic!("{ROW}: {id}: its code {code} lists no severity"));
            let mut base = serde_json::Map::new();
            base.insert("code".to_owned(), Value::String(code.to_owned()));
            base.insert("severity".to_owned(), Value::String(first.clone()));
            base.insert("requirement_id".to_owned(), Value::String(ROW.to_owned()));
            base.insert("message".to_owned(), Value::String("m".to_owned()));
            base
        } else if let Some(variant) = carrier.strip_prefix("ReadError::") {
            golden_variant("read-errors.json", variant)
        } else if let Some(variant) = carrier.strip_prefix("WriteError::") {
            golden_variant("write-errors.json", variant)
        } else {
            panic!("{ROW}: {id}: a carrier this test does not know: {carrier:?}")
        };
        record.insert("code".to_owned(), Value::String(code.to_owned()));
        record.insert("template".to_owned(), Value::String(id.to_owned()));
        record.insert("arguments".to_owned(), Value::Object(arguments.clone()));
        let switched = arguments.iter().next().map(|(name, value)| {
            let other = if value.is_string() {
                Value::from(1_u64)
            } else {
                Value::String("t".to_owned())
            };
            let mut arguments = arguments.clone();
            arguments.insert(name.clone(), other);
            let mut switched = record.clone();
            switched.insert("arguments".to_owned(), Value::Object(arguments));
            Value::Object(switched)
        });
        (Value::Object(record), switched)
    }

    /// The `(name, kind)` of each argument of `arguments`, in order.
    fn argument_kinds(arguments: &[Argument]) -> Vec<(String, String)> {
        arguments
            .iter()
            .map(|argument| {
                (
                    argument.name().to_owned(),
                    kind_of(argument.value()).to_owned(),
                )
            })
            .collect()
    }

    /// `arguments` as the renderer takes them.
    fn named(arguments: &[Argument]) -> Vec<(&str, &ArgumentValue)> {
        arguments
            .iter()
            .map(|argument| (argument.name(), argument.value()))
            .collect()
    }

    /// The template, code and arguments of a value read back, and its
    /// `Display` if it is a refusal.
    struct ReadBack {
        template: String,
        code: Code,
        arguments: Vec<(String, String)>,
        display: Option<String>,
    }

    /// `record` read as its carrier's type, or the refusal as a string.
    fn read_back(carrier: &str, record: Value) -> Result<ReadBack, serde_json::Error> {
        if carrier == "Diagnostic" {
            let read: Diagnostic = serde_json::from_value(record)?;
            Ok(ReadBack {
                template: read.template().to_owned(),
                code: read.code(),
                arguments: argument_kinds(read.arguments()),
                display: None,
            })
        } else if carrier.starts_with("ReadError::") {
            let read: ReadError = serde_json::from_value(record)?;
            Ok(ReadBack {
                template: read.template().to_owned(),
                code: read.code(),
                arguments: argument_kinds(read.arguments()),
                display: Some(read.to_string()),
            })
        } else {
            let read: WriteError = serde_json::from_value(record)?;
            Ok(ReadBack {
                template: read.template().to_owned(),
                code: read.code(),
                arguments: argument_kinds(read.arguments()),
                display: Some(read.to_string()),
            })
        }
    }

    #[test]
    fn lib_diag_006_every_catalog_template_reads_back_as_a_record_of_its_carrier() {
        let catalog = catalog();
        let severities = severities(&catalog);
        let templates = list(&catalog, "templates");
        assert_eq!(templates.len(), 217, "{ROW}: the number of templates");
        let mut switched_count = 0;
        for entry in templates {
            let id = string(entry, "id");
            let carrier = string(entry, "carrier");
            let (record, switched) = records(entry, &severities);
            let read = match read_back(carrier, record.clone()) {
                Ok(read) => read,
                Err(error) => panic!("{ROW}: {id}: {record} does not read as {carrier}: {error}"),
            };
            assert_eq!(read.template, id, "{ROW}: {id}: template()");
            assert_eq!(
                read.code.as_str(),
                string(entry, "code"),
                "{ROW}: {id}: code()"
            );
            let listed = entry_arguments(entry);
            assert_eq!(
                read.arguments, listed,
                "{ROW}: {id}: the argument names and kinds"
            );
            if let Some(display) = &read.display {
                let values: Vec<(String, ArgumentValue)> = listed
                    .iter()
                    .map(|(name, kind)| (name.clone(), test_argument(kind)))
                    .collect();
                let arguments: Vec<(&str, &ArgumentValue)> = values
                    .iter()
                    .map(|(name, value)| (name.as_str(), value))
                    .collect();
                let english = render(string(entry, "english"), &arguments);
                assert!(
                    display.contains(&english),
                    "{ROW}: {id}: the Display {display:?} does not hold the English rendered: \
                     {english:?}"
                );
            }
            if let Some(switched) = switched {
                switched_count += 1;
                match read_back(carrier, switched.clone()) {
                    Ok(_) => {
                        panic!("{ROW}: {id}: {switched} was read, an argument of the other type")
                    }
                    Err(error) => assert!(
                        error.is_data(),
                        "{ROW}: {id}: refused as malformed JSON, not as data: {error}"
                    ),
                }
            }
        }
        assert!(
            switched_count > 0,
            "{ROW}: setup: no template has an argument"
        );
    }

    // -----------------------------------------------------------------------
    // The fixtures
    // -----------------------------------------------------------------------

    /// What the fixtures gave, by carrier kind.
    #[derive(Default)]
    struct Seen {
        diagnostics: usize,
        read_refusals: usize,
        write_refusals: usize,
        edit_refusals: usize,
    }

    /// `diagnostic` matches its catalog entry.
    fn check_diagnostic(
        context: &str,
        catalog: &Value,
        severities: &BTreeMap<String, Vec<String>>,
        diagnostic: &Diagnostic,
    ) {
        let id = diagnostic.template();
        let entry = template_entry(catalog, id);
        assert_eq!(
            string(entry, "carrier"),
            "Diagnostic",
            "{ROW}: {context}: {id}: the carrier of a diagnostic's template"
        );
        assert_eq!(
            diagnostic.code().as_str(),
            string(entry, "code"),
            "{ROW}: {context}: {id}: the code"
        );
        assert_eq!(
            argument_kinds(diagnostic.arguments()),
            entry_arguments(entry),
            "{ROW}: {context}: {id}: the argument names and kinds"
        );
        assert_eq!(
            diagnostic.message(),
            render(string(entry, "english"), &named(diagnostic.arguments())),
            "{ROW}: {context}: {id}: the message is not the English rendered"
        );
        let severity = diagnostic.severity().as_str();
        let listed = severities.get(diagnostic.code().as_str());
        assert!(
            listed.is_some_and(|listed| listed.iter().any(|s| s == severity)),
            "{ROW}: {context}: {id}: the severity {severity} is not one the catalog lists for \
             {}: {listed:?}",
            diagnostic.code()
        );
    }

    /// A refusal as the checks see it.
    struct Refusal<'r> {
        /// `ReadError` or `WriteError`.
        owner: &'static str,
        /// The refusal serialized.
        serialized: Value,
        code: Code,
        template: &'r str,
        arguments: &'r [Argument],
        display: String,
    }

    /// `refusal` matches its catalog entry, and its code is listed with
    /// `error`.
    fn check_refusal(
        context: &str,
        catalog: &Value,
        severities: &BTreeMap<String, Vec<String>>,
        refusal: &Refusal<'_>,
    ) {
        let template = refusal.template;
        let entry = template_entry(catalog, template);
        let variant = string(&refusal.serialized, "type");
        assert_eq!(
            string(entry, "carrier"),
            format!("{}::{variant}", refusal.owner),
            "{ROW}: {context}: {template}: the carrier"
        );
        assert_eq!(
            refusal.code.as_str(),
            string(entry, "code"),
            "{ROW}: {context}: {template}: the code"
        );
        assert_eq!(
            argument_kinds(refusal.arguments),
            entry_arguments(entry),
            "{ROW}: {context}: {template}: the argument names and kinds"
        );
        let english = render(string(entry, "english"), &named(refusal.arguments));
        assert!(
            refusal.display.contains(&english),
            "{ROW}: {context}: {template}: the Display {:?} does not hold {english:?}",
            refusal.display
        );
        let code = refusal.code;
        assert!(
            severities
                .get(code.as_str())
                .is_some_and(|listed| listed.iter().any(|s| s == "error")),
            "{ROW}: {context}: {template}: the catalog does not list error for {code}"
        );
    }

    fn check_read_error(
        context: &str,
        catalog: &Value,
        severities: &BTreeMap<String, Vec<String>>,
        error: &ReadError,
    ) {
        let refusal = Refusal {
            owner: "ReadError",
            serialized: to_value(error),
            code: error.code(),
            template: error.template(),
            arguments: error.arguments(),
            display: error.to_string(),
        };
        check_refusal(context, catalog, severities, &refusal);
    }

    fn check_write_error(
        context: &str,
        catalog: &Value,
        severities: &BTreeMap<String, Vec<String>>,
        error: &WriteError,
    ) {
        let refusal = Refusal {
            owner: "WriteError",
            serialized: to_value(error),
            code: error.code(),
            template: error.template(),
            arguments: error.arguments(),
            display: error.to_string(),
        };
        check_refusal(context, catalog, severities, &refusal);
        if let WriteError::Invalid { diagnostics, .. } = error {
            for diagnostic in diagnostics {
                check_diagnostic(context, catalog, severities, diagnostic);
            }
        }
    }

    /// Every `.xmi` file under `dir`, sorted.
    fn xmi_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(next) = pending.pop() {
            let entries = std::fs::read_dir(&next)
                .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", next.display()));
            for entry in entries {
                let entry = entry.unwrap_or_else(|error| {
                    panic!("{ROW}: cannot list {}: {error}", next.display())
                });
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "xmi") {
                    found.push(path);
                }
            }
        }
        found.sort();
        found
    }

    /// The documents of slice 0082's two `UnrepresentableCharacter` cases:
    /// U+0001 in a package's name, and U+0004 in the second value of a
    /// Note's content.
    fn unrepresentable_documents() -> Vec<Document> {
        let apply = |doc: &mut Document, operation: Operation| {
            if let Err(error) = edit::apply(doc, operation) {
                panic!("{ROW}: setup: {error}");
            }
        };
        let mut in_name = Document::new();
        apply(
            &mut in_name,
            CreateArgumentPackage::root(LangString::new("bad\u{1}"))
                .with_id("arguments")
                .into(),
        );
        let mut in_value = Document::new();
        let content: MultiLangString = [LangString::new("ok"), LangString::new("bad\u{4}")]
            .into_iter()
            .collect();
        apply(
            &mut in_value,
            CreateArgumentPackage::root(LangString::new("P"))
                .with_id("p")
                .into(),
        );
        apply(
            &mut in_value,
            CreateNote::new("p", content).with_id("n1").into(),
        );
        vec![in_name, in_value]
    }

    /// Writes `doc` in both modes and checks each refusal.
    fn check_writes(
        context: &str,
        catalog: &Value,
        sev: &BTreeMap<String, Vec<String>>,
        doc: &Document,
        seen: &mut Seen,
    ) {
        for mode in [Mode::Strict, Mode::Tolerant] {
            if let Err(error) = xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
                seen.write_refusals += 1;
                check_write_error(
                    &format!("{context}, written {mode:?}"),
                    catalog,
                    sev,
                    &error,
                );
            }
        }
    }

    /// A set of edits each refused: their codes are listed with `error`.
    fn check_edit_refusals(sev: &BTreeMap<String, Vec<String>>, seen: &mut Seen) {
        let mut doc = Document::new();
        for operation in [
            Operation::from(CreateArgumentPackage::root(LangString::new("P")).with_id("p")),
            CreateClaim::new("p", LangString::new("C1"))
                .with_id("c1")
                .into(),
            CreateClaim::new("p", LangString::new("C2"))
                .with_id("c2")
                .into(),
            CreateAssertedInference::new("p", LangString::new("I1"), ["c2"], "c1")
                .with_id("i1")
                .into(),
        ] {
            if let Err(error) = edit::apply(&mut doc, operation) {
                panic!("{ROW}: setup: {error}");
            }
        }
        let refused: Vec<Operation> = vec![
            SetName::new("nowhere", LangString::new("N")).into(),
            CreateClaim::new("p", LangString::new("C"))
                .with_id("c1")
                .into(),
            CreateClaim::new("c1", LangString::new("C")).into(),
            CreateClaim::new("p", LangString::new("C"))
                .with_id("1 bad")
                .into(),
            SetContent::new("c1", MultiLangString::from(LangString::new("x"))).into(),
            ReorderElements::new("p", ["c1", "c1"]).into(),
            DeleteElement::new("c1").into(),
            DeleteElement::new("p").into(),
        ];
        let mut codes: Vec<Code> = Vec::new();
        for operation in refused {
            match edit::check(&doc, &operation) {
                Ok(()) => panic!("{ROW}: setup: {operation:?} is accepted"),
                Err(error) => codes.push(error.code()),
            }
        }
        // A preview of a document that has changed since.
        let stale = Operation::from(SetName::new("c1", LangString::new("X")));
        let preview =
            edit::preview(&doc, &stale).unwrap_or_else(|error| panic!("{ROW}: setup: {error}"));
        if let Err(error) = edit::apply(&mut doc, SetName::new("c2", LangString::new("Y"))) {
            panic!("{ROW}: setup: {error}");
        }
        match edit::apply_preview(&mut doc, &preview) {
            Ok(mutation) => panic!("{ROW}: setup: a stale preview applied: {mutation:?}"),
            Err(error) => codes.push(error.code()),
        }
        for code in codes {
            seen.edit_refusals += 1;
            assert!(
                sev.get(code.as_str())
                    .is_some_and(|listed| listed.iter().any(|s| s == "error")),
                "{ROW}: an EditError's code {code} is not listed with error"
            );
        }
    }

    #[test]
    fn lib_diag_006_every_diagnostic_and_refusal_of_the_fixtures_matches_its_catalog_entry() {
        let catalog = catalog();
        let sev = severities(&catalog);
        let mut seen = Seen::default();
        let files = xmi_files(std::path::Path::new(DATA));
        assert!(!files.is_empty(), "{ROW}: setup: no .xmi under {DATA}");
        for path in &files {
            let name = path
                .strip_prefix(DATA)
                .unwrap_or(path)
                .display()
                .to_string();
            let bytes = std::fs::read(path)
                .unwrap_or_else(|error| panic!("{ROW}: cannot read {name}: {error}"));
            for mode in [Mode::Strict, Mode::Tolerant] {
                let context = format!("{name} read {mode:?}");
                match xmi::read(&bytes, &ReadOptions::default().with_mode(mode)) {
                    Ok(output) => {
                        let validated = validation::validate(output.document());
                        for diagnostic in output.diagnostics().iter().chain(&validated) {
                            seen.diagnostics += 1;
                            check_diagnostic(&context, &catalog, &sev, diagnostic);
                        }
                        check_writes(&context, &catalog, &sev, output.document(), &mut seen);
                    }
                    Err(error) => {
                        seen.read_refusals += 1;
                        check_read_error(&context, &catalog, &sev, &error);
                    }
                }
            }
        }
        for (index, doc) in unrepresentable_documents().iter().enumerate() {
            let before = seen.write_refusals;
            check_writes(
                &format!("unrepresentable case {index}"),
                &catalog,
                &sev,
                doc,
                &mut seen,
            );
            assert!(
                seen.write_refusals > before,
                "{ROW}: setup: unrepresentable case {index} is written"
            );
        }
        check_edit_refusals(&sev, &mut seen);
        for (what, count) in [
            ("diagnostic", seen.diagnostics),
            ("ReadError", seen.read_refusals),
            ("WriteError", seen.write_refusals),
            ("EditError", seen.edit_refusals),
        ] {
            assert!(count > 0, "{ROW}: setup: no {what} was seen");
        }
    }
}
