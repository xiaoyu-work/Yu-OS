//! Progressive, read-only discovery of the public `yu` command tree.
//!
//! The tool consumes the same command catalogue as terminal help, but accepts
//! only structural path segments. It cannot express flags, operands, hidden
//! `__*` routes, or an operational invocation.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolResult};

pub struct YuHelp;

#[async_trait]
impl Tool for YuHelp {
    fn name(&self) -> &str {
        "yu_help"
    }

    fn description(&self) -> &str {
        "Progressively explore the machine-readable `yu` CLI command tree \
         without running commands. Start with path=[]; follow a returned \
         namespace with path=[\"agent\"], then inspect a command with \
         path=[\"agent\",\"usage\"]. Path `[\"app\"]` describes the human CLI; \
         use `yu_tool_search` for permitted MCP App services. Before claiming \
         Yu OS lacks a capability, \
         inspect the relevant path here. Operational work must use the \
         returned model_tool or another named, capability-gated tool."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "pattern": "^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$"
                    },
                    "maxItems": 4,
                    "default": [],
                    "description": "Command names after `yu`, one level at a time. Empty lists root namespaces; [\"agent\"] lists Agent commands; [\"agent\",\"usage\"] describes that command."
                }
            },
            "additionalProperties": false
        })
    }

    async fn exec(&self, input: Value) -> ToolResult {
        let path = match parse_path(&input) {
            Ok(path) => path,
            Err(error) => return ToolResult::err(error),
        };
        ToolResult::ok(discover(&path).to_string())
    }

    fn parallel_safe(&self) -> bool {
        true
    }
}

fn parse_path(input: &Value) -> Result<Vec<String>, String> {
    let object = input
        .as_object()
        .ok_or_else(|| "yu_help input must be an object".to_string())?;
    if object.keys().any(|key| key != "path") {
        return Err("yu_help accepts only the `path` field".to_string());
    }
    let Some(raw) = object.get("path") else {
        return Ok(Vec::new());
    };
    let values = raw
        .as_array()
        .ok_or_else(|| "`path` must be an array of command names".to_string())?;
    if values.len() > 4 {
        return Err("`path` supports at most four command levels".to_string());
    }
    values
        .iter()
        .map(|value| {
            let segment = value
                .as_str()
                .ok_or_else(|| "every `path` segment must be a string".to_string())?;
            if !valid_segment(segment) {
                return Err(format!(
                    "invalid command path segment `{segment}`; flags, operands, and hidden routes are not discovery paths"
                ));
            }
            Ok(segment.to_string())
        })
        .collect()
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 64
        && segment
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        && !segment.contains("..")
}

fn discover(path: &[String]) -> Value {
    match path {
        [] => {
            let mut overview = crate::cli_catalog::overview(env!("CARGO_PKG_VERSION"), 0);
            if let Some(object) = overview.as_object_mut() {
                object.remove("apps_available");
            }
            overview
        }
        [namespace, command] if namespace == "agent" && command == "dev" => discover_agent_dev(),
        [namespace, command, subcommand] if namespace == "agent" && command == "dev" => {
            discover_agent_dev_command(subcommand)
        }
        [namespace, command] if namespace == "agent" && command == "setup" => {
            discover_agent_setup()
        }
        [namespace, command, node] if namespace == "agent" && command == "setup" => {
            discover_agent_setup_node(node)
        }
        [namespace, command, modality, subcommand]
            if namespace == "agent" && command == "setup" =>
        {
            discover_agent_setup_subcommand(modality, subcommand)
        }
        [namespace] if namespace == "app" => discover_apps(),
        [namespace] => crate::cli_catalog::namespace_help(namespace)
            .map(|mut value| {
                value["found"] = json!(true);
                value["path"] = json!(path);
                value["kind"] = json!("namespace");
                value
            })
            .unwrap_or_else(|| {
                json!({
                    "found": false,
                    "path": path,
                    "available": crate::cli_catalog::namespace_names(),
                })
            }),
        [namespace, command] if namespace == "app" => discover_app(command),
        [namespace, command] => {
            if let Some(value) = discover_nested_builtin(path) {
                return value;
            }
            discover_builtin_command(namespace, command, path)
        }
        [namespace, app, command] if namespace == "app" => discover_app_command(app, command),
        [_, _, _] | [_, _, _, _] => discover_nested_builtin(path).unwrap_or_else(|| {
            json!({
                "found": false,
                "path": path,
                "reason": "the selected command has no discoverable child with this name",
            })
        }),
        _ => json!({
            "found": false,
            "path": path,
            "reason": "the command path is not part of the public CLI tree",
        }),
    }
}

fn discover_builtin_command(namespace: &str, command: &str, path: &[String]) -> Value {
    let detailed = crate::cli_help::show_command_schema(namespace, command)
        .ok()
        .flatten()
        .and_then(|encoded| serde_json::from_str::<Value>(&encoded).ok());
    detailed
        .or_else(|| crate::cli_catalog::command_help(namespace, command))
        .map(|mut value| {
            value["found"] = json!(true);
            value["path"] = json!(path);
            value["kind"] = json!("command");
            value
        })
        .unwrap_or_else(|| {
            json!({
                "found": false,
                "path": path,
                "available": crate::cli_catalog::command_names(namespace),
            })
        })
}

fn discover_apps() -> Value {
    json!({
        "found": true,
        "path": ["app"],
        "kind": "namespace",
        "management": {
            "stdio": crate::cli_catalog::APP_STDIO_DESCRIPTION,
            "lint": "Validate installed App manifests and source policy",
            "tool": "Inspect App session tools",
            "install": "Install an App from a source directory",
            "create": "Scaffold a new App",
            "consent": "Inspect and manage App AI consent",
        },
        "model_tool": "yu_tool_search",
        "next": "Search permitted MCP App tools with yu_tool_search.",
    })
}

fn discover_app(app_id: &str) -> Value {
    if app_id == "stdio" {
        return discover_builtin_command("app", "stdio", &["app".to_string(), "stdio".to_string()]);
    }
    if let Some((description, subcommands)) = app_management(app_id) {
        return json!({
            "found": true,
            "path": ["app", app_id],
            "kind": if subcommands.is_empty() { "command" } else { "namespace" },
            "command": format!("yu app {app_id}"),
            "description": description,
            "subcommands": subcommands,
            "model_callable": false,
            "model_tool": Value::Null,
        });
    }
    json!({
        "found": false,
        "path": ["app", app_id],
        "model_tool": "yu_tool_search",
        "reason": "Installed App tools are discoverable only through the MCP tool catalogue.",
    })
}

fn discover_app_command(app_id: &str, command: &str) -> Value {
    if let Some((description, subcommands)) = app_management(app_id) {
        if !subcommands.contains(&command) {
            return json!({
                "found": false,
                "path": ["app", app_id, command],
                "available": subcommands,
            });
        }
        return json!({
            "found": true,
            "path": ["app", app_id, command],
            "kind": "command",
            "command": format!("yu app {app_id} {command}"),
            "description": description,
            "model_callable": false,
            "model_tool": Value::Null,
        });
    }
    json!({
        "found": false,
        "path": ["app", app_id, command],
        "reason": "Installed App tools are discoverable only through the MCP tool catalogue.",
    })
}

fn discover_agent_dev() -> Value {
    let mut help = crate::agent::dev_help();
    help["found"] = json!(true);
    help["path"] = json!(["agent", "dev"]);
    help["kind"] = json!("namespace");
    help["stability"] = json!("internal");
    help
}

fn discover_agent_dev_command(command: &str) -> Value {
    let help = crate::agent::dev_help();
    let subcommands = help["subcommands"].as_array().cloned().unwrap_or_default();
    if !subcommands
        .iter()
        .any(|entry| entry.as_str() == Some(command))
    {
        return json!({
            "found": false,
            "path": ["agent", "dev", command],
            "available": subcommands,
        });
    }
    let canonical = (command == "usage").then_some("yu agent usage");
    json!({
        "found": true,
        "path": ["agent", "dev", command],
        "kind": "command",
        "command": format!("yu agent dev {command}"),
        "canonical_command": canonical,
        "stability": "internal",
        "model_callable": command == "usage",
        "model_tool": (command == "usage").then_some("yu_usage"),
    })
}

fn discover_agent_setup() -> Value {
    let mut help = crate::agent::setup::help_doc();
    help["found"] = json!(true);
    help["path"] = json!(["agent", "setup"]);
    help["kind"] = json!("namespace");
    help
}

fn discover_agent_setup_node(node: &str) -> Value {
    let help = crate::agent::setup::help_doc();
    if let Some(description) = help["modalities"].get(node) {
        return json!({
            "found": true,
            "path": ["agent", "setup", node],
            "kind": "namespace",
            "command": format!("yu agent setup {node}"),
            "description": description,
            "subcommands": help["subcommands"],
            "flags": help["flags"],
            "model_callable": false,
        });
    }
    if let Some(description) = help["subcommands"].get(node) {
        return json!({
            "found": true,
            "path": ["agent", "setup", node],
            "kind": "command",
            "command": format!("yu agent setup {node}"),
            "description": description,
            "model_callable": false,
        });
    }
    json!({
        "found": false,
        "path": ["agent", "setup", node],
        "available": {
            "modalities": help["modalities"],
            "subcommands": help["subcommands"],
        },
    })
}

fn discover_agent_setup_subcommand(modality: &str, subcommand: &str) -> Value {
    let help = crate::agent::setup::help_doc();
    if help["modalities"].get(modality).is_none() {
        return json!({
            "found": false,
            "path": ["agent", "setup", modality, subcommand],
            "available": help["modalities"],
        });
    }
    let Some(description) = help["subcommands"].get(subcommand) else {
        return json!({
            "found": false,
            "path": ["agent", "setup", modality, subcommand],
            "available": help["subcommands"],
        });
    };
    json!({
        "found": true,
        "path": ["agent", "setup", modality, subcommand],
        "kind": "command",
        "command": format!("yu agent setup {modality} {subcommand}"),
        "description": description,
        "flags": help["flags"],
        "model_callable": false,
    })
}

fn discover_nested_builtin(path: &[String]) -> Option<Value> {
    let path_refs: Vec<&str> = path.iter().map(String::as_str).collect();
    if let Some(children) = crate::cli_catalog::nested_commands(&path_refs) {
        let usage_scope = path_refs.as_slice() == ["agent", "usage"];
        return Some(json!({
            "found": true,
            "path": path,
            "kind": "namespace",
            "command": format!("yu {}", path.join(" ")),
            "subcommands": children
                .into_iter()
                .map(|(name, description)| json!({
                    "name": name,
                    "description": description,
                }))
                .collect::<Vec<_>>(),
            "model_callable": usage_scope,
            "model_tool": usage_scope.then_some("yu_usage"),
        }));
    }
    let (name, parent) = path.split_last()?;
    let parent_refs: Vec<&str> = parent.iter().map(String::as_str).collect();
    let siblings = crate::cli_catalog::nested_commands(&parent_refs)?;
    let Some((_, description)) = siblings
        .iter()
        .find(|(candidate, _)| *candidate == name.as_str())
    else {
        return Some(json!({
            "found": false,
            "path": path,
            "available": siblings
                .into_iter()
                .map(|(candidate, _)| candidate)
                .collect::<Vec<_>>(),
        }));
    };
    let usage_scope =
        parent.len() == 2 && parent[0].as_str() == "agent" && parent[1].as_str() == "usage";
    Some(json!({
        "found": true,
        "path": path,
        "kind": "command",
        "command": format!("yu {}", path.join(" ")),
        "description": description,
        "model_callable": usage_scope,
        "model_tool": usage_scope.then_some("yu_usage"),
    }))
}

fn app_management(name: &str) -> Option<(&'static str, &'static [&'static str])> {
    match name {
        "stdio" => Some((crate::cli_catalog::APP_STDIO_DESCRIPTION, &[])),
        "lint" => Some(("Validate installed App manifests and source policy", &[])),
        "install" => Some(("Install an App from a source directory", &[])),
        "create" => Some(("Scaffold a new App", &[])),
        "tool" => Some(("Inspect App session tools", &["list"])),
        "consent" => Some((
            "Inspect and manage App AI consent",
            &["list", "show", "path", "grant", "revoke"],
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/agent/tools/yu_help.rs"
    ));
}
