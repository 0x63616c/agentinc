//! Lint the checked-in OpenAPI document (`api/openapi-3.0.json`, which `cargo xtask
//! generate` keeps equal to what the daemon serves), so the API seam keeps the
//! product's vocabulary and one error contract:
//!
//! - every operation ID is `{resource}_{action}`, with the resource a noun from
//!   `CONTEXT.md` (singular or plural), `health` or `version`;
//! - every versioned operation declares the shared 401, 426 and 500 responses
//!   and every error response is an `ErrorBody`;
//! - no schema or field name uses a word `CONTEXT.md` says to avoid everywhere;
//!
//! Names the plan renames in a later work package are listed in `LEGACY_*` with
//! the package that retires them; nothing new may join those lists.
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::Value;
use std::{fs, path::Path};

/// Operation IDs whose resource is not yet a CONTEXT.md noun.
const LEGACY_OPERATIONS: &[(&str, &str)] = &[
    ("product_state", "WP-D6 moves /v1/state to resource paths"),
    (
        "product_command",
        "WP-D6 moves /v1/commands to resource paths",
    ),
    ("terminal_sessions_list", "terminal is not a construct yet"),
    (
        "terminal_sessions_create",
        "terminal is not a construct yet",
    ),
    ("terminal_sessions_close", "terminal is not a construct yet"),
];
/// Schema names carrying an avoid-word until their rename lands.
const LEGACY_SCHEMAS: &[(&str, &str)] = &[
    (
        "TerminalSession",
        "rename to Terminal with the Mac terminal page",
    ),
    (
        "CreateTerminalSession",
        "rename to CreateTerminal with the Mac terminal page",
    ),
];
const SHARED_ERRORS: [&str; 3] = ["401", "426", "500"];

pub fn run(root: &Path) -> Result<()> {
    let context = fs::read_to_string(root.join("CONTEXT.md"))?;
    let api: Value = serde_json::from_slice(&fs::read(root.join("api/openapi-3.0.json"))?)?;
    let problems = lint(&api, &nouns(&context), &avoid_words(&context));
    if problems.is_empty() {
        println!("OpenAPI document follows the API seam rules");
        return Ok(());
    }
    for problem in &problems {
        println!("::error::openapi: {problem}");
    }
    bail!("{} OpenAPI seam violation(s)", problems.len());
}

/// The construct names in CONTEXT.md's Language section, lower-cased.
fn nouns(context: &str) -> Vec<String> {
    let term = Regex::new(r"^\*\*([^*]+)\*\*:\s*$").unwrap();
    let mut in_language = false;
    let mut found = Vec::new();
    for line in context.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_language = heading.trim() == "Language";
            continue;
        }
        if in_language && let Some(captures) = term.captures(line) {
            found.push(captures[1].trim().to_lowercase());
        }
    }
    found
}

/// The words CONTEXT.md says to avoid everywhere, lower-cased.
fn avoid_words(context: &str) -> Vec<String> {
    let Some(section) = context.split("## Avoid everywhere").nth(1) else {
        return Vec::new();
    };
    let body = section.split("\n## ").next().unwrap_or(section);
    let word = Regex::new(r"^\s*([A-Za-z]+)").unwrap();
    body.split(',')
        .filter_map(|item| word.captures(item).map(|c| c[1].to_lowercase()))
        .filter(|w| !w.is_empty())
        .collect()
}

fn lint(api: &Value, nouns: &[String], avoid: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let resource_ok = |resource: &str| {
        resource == "health"
            || resource == "version"
            || nouns.iter().any(|noun| {
                let noun = noun.replace(' ', "_");
                resource == noun || resource == format!("{noun}s") || resource == plural_y(&noun)
            })
    };
    for (path, item) in object(&api["paths"]) {
        for (method, operation) in object(item) {
            let id = operation["operationId"].as_str().unwrap_or("");
            let legacy = LEGACY_OPERATIONS.iter().any(|(legacy, _)| *legacy == id);
            match id.split_once('_') {
                Some((resource, action))
                    if !action.is_empty()
                        && id.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                        && (legacy || resource_ok(resource)) => {}
                _ => problems.push(format!(
                    "{method} {path}: operationId `{id}` must be `{{resource}}_{{action}}` with a CONTEXT.md noun"
                )),
            }
            let responses = &operation["responses"];
            if path.starts_with("/v1/") {
                for status in SHARED_ERRORS {
                    if responses[status].is_null() {
                        problems.push(format!("{method} {path}: missing shared {status} response"));
                    }
                }
            }
            for (status, response) in object(responses) {
                if status.starts_with('2') || response["$ref"].is_string() {
                    continue;
                }
                let schema = &response["content"]["application/json"]["schema"]["$ref"];
                if schema != "#/components/schemas/ErrorBody" {
                    problems.push(format!(
                        "{method} {path}: {status} response is not an ErrorBody"
                    ));
                }
            }
        }
    }
    for (name, schema) in object(&api["components"]["schemas"]) {
        if LEGACY_SCHEMAS.iter().any(|(legacy, _)| legacy == name) {
            continue;
        }
        if let Some(word) = avoided(name, avoid) {
            problems.push(format!("schema `{name}` uses the avoided word `{word}`"));
        }
        for field in property_names(schema) {
            if let Some(word) = avoided(&field, avoid) {
                problems.push(format!(
                    "schema `{name}` field `{field}` uses the avoided word `{word}`"
                ));
            }
        }
    }
    problems
}

fn plural_y(noun: &str) -> String {
    noun.strip_suffix('y')
        .map(|stem| format!("{stem}ies"))
        .unwrap_or_default()
}

fn object(value: &Value) -> impl Iterator<Item = (&String, &Value)> {
    value.as_object().into_iter().flatten()
}

/// Every property name anywhere inside a schema, including nested variants.
fn property_names(schema: &Value) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![schema];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(map) => {
                if let Some(Value::Object(properties)) = map.get("properties") {
                    names.extend(properties.keys().cloned());
                }
                pending.extend(map.values());
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    names
}

/// The avoided word a camel- or snake-case name contains, if any.
fn avoided(name: &str, avoid: &[String]) -> Option<String> {
    let words = Regex::new(r"[A-Z]?[a-z0-9]+|[A-Z]+").unwrap();
    words
        .find_iter(name)
        .map(|word| word.as_str().to_lowercase())
        .find(|word| avoid.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CONTEXT: &str = "# X\n\n## Language\n\n**Ticket**:\nWork.\n_Avoid_: task\n\n**Activity**:\nA change.\n\n## Reserved words\n\n**Session**:\nSDK only.\n\n## Avoid everywhere\n\nagentic (say what the agents do), principal (say Owner), session (outside the SDK), firstmate.\n";

    #[test]
    fn context_supplies_nouns_and_avoid_words() {
        assert_eq!(nouns(CONTEXT), ["ticket", "activity"]);
        assert_eq!(
            avoid_words(CONTEXT),
            ["agentic", "principal", "session", "firstmate"]
        );
    }

    #[test]
    fn violations_are_named_and_a_clean_document_passes() {
        let nouns = nouns(CONTEXT);
        let avoid = avoid_words(CONTEXT);
        let error = json!({"content":{"application/json":{"schema":{"$ref":"#/components/schemas/ErrorBody"}}}});
        let shared = json!({"401":{"$ref":"#/components/responses/Unauthorized"},"426":{"$ref":"#/components/responses/UpgradeRequired"},"500":{"$ref":"#/components/responses/Internal"}});
        let mut ok = shared.clone();
        ok["200"] = json!({});
        ok["409"] = error.clone();
        let clean = json!({
            "paths": {"/v1/tickets": {"get": {"operationId": "tickets_state", "responses": ok}},
                      "/v1/tickets/{id}/activity": {"get": {"operationId": "activities_list", "responses": shared}}},
            "components": {"schemas": {"TicketSnapshot": {"properties": {"ticket_id": {}}}}}
        });
        assert_eq!(lint(&clean, &nouns, &avoid), Vec::<String>::new());
        let dirty = json!({
            "paths": {"/v1/sessions": {"get": {"operationId": "getSessions", "responses": {"404": {"description": "bare"}}}}},
            "components": {"schemas": {"UserSession": {"properties": {"principal_id": {}}}}}
        });
        let problems = lint(&dirty, &nouns, &avoid);
        assert_eq!(problems.len(), 7, "{problems:#?}");
        assert!(problems[0].contains("`getSessions`"));
        assert!(problems.iter().any(|p| p.contains("missing shared 426")));
        assert!(
            problems
                .iter()
                .any(|p| p.contains("404 response is not an ErrorBody"))
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("schema `UserSession` uses the avoided word `session`"))
        );
        assert!(problems.iter().any(|p| p.contains("field `principal_id`")));
    }

    #[test]
    fn the_served_document_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        run(&root).unwrap();
    }
}
