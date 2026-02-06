use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct CompletionCandidate {
    pub label: String,
    pub insert_text: String,
    pub detail: String,
    pub is_snippet: bool,
}

#[derive(Clone, Debug)]
pub struct DiagnosticCandidate {
    pub line: usize,
    pub severity: u8,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub completions: Vec<CompletionCandidate>,
    pub diagnostics: Vec<DiagnosticCandidate>,
    pub formatted_text: Option<String>,
    pub definitions: Vec<String>,
    pub references: Vec<String>,
    pub implementations: Vec<String>,
    pub had_server: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RequestKind {
    pub want_completion: bool,
    pub want_formatting: bool,
    pub want_definition: bool,
    pub want_references: bool,
    pub want_implementations: bool,
}

pub fn collect_snapshot(
    path: &Path,
    text: &str,
    line: usize,
    col: usize,
    request: RequestKind,
) -> Snapshot {
    let Some(server) = resolve_server(path) else {
        return Snapshot {
            completions: snippets_for(path),
            diagnostics: Vec::new(),
            formatted_text: None,
            definitions: Vec::new(),
            references: Vec::new(),
            implementations: Vec::new(),
            had_server: false,
        };
    };

    let uri = format!("file://{}", path.to_string_lossy());
    let mut messages: Vec<Value> = Vec::new();
    let mut id = 1u64;

    messages.push(request_msg(
        id,
        "initialize",
        json!({
            "processId": std::process::id(),
            "rootUri": Value::Null,
            "capabilities": {
                "textDocument": {
                    "completion": {"completionItem": {"snippetSupport": true}}
                }
            }
        }),
    ));
    id += 1;
    messages.push(notification_msg("initialized", json!({})));
    messages.push(notification_msg(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": server.language_id,
                "version": 1,
                "text": text
            }
        }),
    ));

    let completion_id = if request.want_completion {
        let req_id = id;
        id += 1;
        messages.push(request_msg(
            req_id,
            "textDocument/completion",
            json!({
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col}
            }),
        ));
        Some(req_id)
    } else {
        None
    };

    let formatting_id = if request.want_formatting {
        let req_id = id;
        id += 1;
        messages.push(request_msg(
            req_id,
            "textDocument/formatting",
            json!({
                "textDocument": {"uri": uri},
                "options": {
                    "tabSize": 4,
                    "insertSpaces": true,
                    "trimTrailingWhitespace": true,
                    "insertFinalNewline": false
                }
            }),
        ));
        Some(req_id)
    } else {
        None
    };

    let definition_id = if request.want_definition {
        let req_id = id;
        id += 1;
        messages.push(request_msg(
            req_id,
            "textDocument/definition",
            json!({
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col}
            }),
        ));
        Some(req_id)
    } else {
        None
    };
    let references_id = if request.want_references {
        let req_id = id;
        id += 1;
        messages.push(request_msg(
            req_id,
            "textDocument/references",
            json!({
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col},
                "context": {"includeDeclaration": true}
            }),
        ));
        Some(req_id)
    } else {
        None
    };
    let implementations_id = if request.want_implementations {
        let req_id = id;
        id += 1;
        messages.push(request_msg(
            req_id,
            "textDocument/implementation",
            json!({
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col}
            }),
        ));
        Some(req_id)
    } else {
        None
    };

    let shutdown_id = id;
    messages.push(request_msg(shutdown_id, "shutdown", json!(null)));
    messages.push(notification_msg("exit", json!(null)));

    let mut child = match Command::new(&server.program)
        .args(&server.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            return Snapshot {
                completions: snippets_for(path),
                diagnostics: Vec::new(),
                formatted_text: None,
                definitions: Vec::new(),
                references: Vec::new(),
                implementations: Vec::new(),
                had_server: false,
            }
        }
    };

    if let Some(stdin) = child.stdin.as_mut() {
        for message in messages {
            let _ = write_lsp_message(&mut *stdin, &message);
        }
    }

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(_) => {
            return Snapshot {
                completions: snippets_for(path),
                diagnostics: Vec::new(),
                formatted_text: None,
                definitions: Vec::new(),
                references: Vec::new(),
                implementations: Vec::new(),
                had_server: false,
            }
        }
    };

    let mut snapshot = Snapshot {
        completions: snippets_for(path),
        diagnostics: Vec::new(),
        formatted_text: None,
        definitions: Vec::new(),
        references: Vec::new(),
        implementations: Vec::new(),
        had_server: true,
    };

    let responses = parse_lsp_stream(&output.stdout);
    for message in responses {
        if let Some(method) = message.get("method").and_then(|v| v.as_str()) {
            if method == "textDocument/publishDiagnostics" {
                if let Some(diags) = message
                    .get("params")
                    .and_then(|p| p.get("diagnostics"))
                    .and_then(|v| v.as_array())
                {
                    for diag in diags {
                        let line = diag
                            .get("range")
                            .and_then(|r| r.get("start"))
                            .and_then(|s| s.get("line"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        let severity =
                            diag.get("severity").and_then(|v| v.as_u64()).unwrap_or(3) as u8;
                        let message = diag
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Diagnostic")
                            .to_string();
                        snapshot.diagnostics.push(DiagnosticCandidate {
                            line,
                            severity,
                            message,
                        });
                    }
                }
            }
            continue;
        }

        let Some(resp_id) = message.get("id").and_then(|v| v.as_u64()) else {
            continue;
        };
        if Some(resp_id) == completion_id {
            if let Some(items) = completion_items_from_result(message.get("result")) {
                snapshot.completions.extend(items);
            }
        } else if Some(resp_id) == formatting_id {
            snapshot.formatted_text = formatting_from_result(message.get("result"), text);
        } else if Some(resp_id) == definition_id {
            snapshot.definitions = locations_from_result(message.get("result"));
        } else if Some(resp_id) == references_id {
            snapshot.references = locations_from_result(message.get("result"));
        } else if Some(resp_id) == implementations_id {
            snapshot.implementations = locations_from_result(message.get("result"));
        } else if resp_id == shutdown_id {
            // no-op
        }
    }

    snapshot
}

fn write_lsp_message(mut writer: impl Write, body: &Value) -> std::io::Result<()> {
    let encoded = body.to_string();
    write!(
        writer,
        "Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}",
        encoded.len(),
        encoded
    )
}

fn parse_lsp_stream(bytes: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let mut header_end = None;
        for i in cursor..bytes.len().saturating_sub(3) {
            if bytes[i..i + 4] == *b"\r\n\r\n" {
                header_end = Some(i + 4);
                break;
            }
        }
        let Some(header_end) = header_end else {
            break;
        };
        let header = String::from_utf8_lossy(&bytes[cursor..header_end]);
        let mut content_len = None;
        for line in header.lines() {
            if let Some(value) = line.strip_prefix("Content-Length:") {
                content_len = value.trim().parse::<usize>().ok();
                break;
            }
        }
        let Some(content_len) = content_len else {
            break;
        };
        let body_start = header_end;
        let body_end = body_start.saturating_add(content_len);
        if body_end > bytes.len() {
            break;
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes[body_start..body_end]) {
            out.push(value);
        }
        cursor = body_end;
    }
    out
}

fn completion_items_from_result(result: Option<&Value>) -> Option<Vec<CompletionCandidate>> {
    let result = result?;
    let items = if let Some(items) = result.as_array() {
        Some(items)
    } else {
        result.get("items").and_then(|v| v.as_array())
    }?;
    let mut out = Vec::new();
    for item in items {
        let label = item
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if label.is_empty() {
            continue;
        }
        let insert_text = item
            .get("insertText")
            .and_then(|v| v.as_str())
            .unwrap_or(&label)
            .to_string();
        let detail = item
            .get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or("LSP")
            .to_string();
        let is_snippet = item
            .get("insertTextFormat")
            .and_then(|v| v.as_u64())
            .map(|v| v == 2)
            .unwrap_or(false);
        out.push(CompletionCandidate {
            label,
            insert_text,
            detail,
            is_snippet,
        });
    }
    Some(out)
}

fn formatting_from_result(result: Option<&Value>, original: &str) -> Option<String> {
    let edits = result?.as_array()?;
    if edits.is_empty() {
        return None;
    }
    let first_new_text = edits
        .first()
        .and_then(|v| v.get("newText"))
        .and_then(|v| v.as_str())?;
    if edits.len() == 1 {
        return Some(first_new_text.to_string());
    }
    // Fallback: if multiple edits arrive, keep original (avoids applying complex ranges incorrectly).
    Some(original.to_string())
}

fn locations_from_result(result: Option<&Value>) -> Vec<String> {
    let Some(result) = result else {
        return Vec::new();
    };
    let entries: Vec<&Value> = if let Some(arr) = result.as_array() {
        arr.iter().collect()
    } else {
        vec![result]
    };
    let mut out = Vec::new();
    for entry in entries {
        let uri = entry.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        let line = entry
            .get("range")
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            + 1;
        let character = entry
            .get("range")
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("character"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            + 1;
        if !uri.is_empty() {
            out.push(format!("{}:{}:{}", uri, line, character));
        }
    }
    out
}

fn request_msg(id: u64, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    })
}

fn notification_msg(method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    })
}

struct ServerSpec {
    program: String,
    args: Vec<String>,
    language_id: &'static str,
}

fn resolve_server(path: &Path) -> Option<ServerSpec> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => Some(ServerSpec {
            program: "rust-analyzer".to_string(),
            args: Vec::new(),
            language_id: "rust",
        }),
        "py" => Some(ServerSpec {
            program: "pylsp".to_string(),
            args: Vec::new(),
            language_id: "python",
        }),
        "go" => Some(ServerSpec {
            program: "gopls".to_string(),
            args: Vec::new(),
            language_id: "go",
        }),
        "js" | "jsx" | "ts" | "tsx" => Some(ServerSpec {
            program: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            language_id: "typescript",
        }),
        _ => None,
    }
}

fn snippets_for(path: &Path) -> Vec<CompletionCandidate> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let raw: &[(&str, &str, &str)] = match ext.as_str() {
        "rs" => &[
            (
                "fn",
                "fn ${1:name}(${2:args}) {\n    ${3}\n}",
                "Rust snippet",
            ),
            ("impl", "impl ${1:Type} {\n    ${2}\n}", "Rust snippet"),
            (
                "test",
                "#[test]\nfn ${1:name}() {\n    ${2}\n}",
                "Rust snippet",
            ),
        ],
        "py" => &[
            (
                "def",
                "def ${1:name}(${2:args}):\n    ${3:pass}",
                "Python snippet",
            ),
            (
                "class",
                "class ${1:Name}:\n    def __init__(self):\n        ${2:pass}",
                "Python snippet",
            ),
        ],
        "js" | "jsx" | "ts" | "tsx" => &[
            (
                "func",
                "function ${1:name}(${2:args}) {\n  ${3}\n}",
                "JS/TS snippet",
            ),
            ("clg", "console.log(${1:value});", "JS/TS snippet"),
        ],
        _ => &[],
    };
    raw.iter()
        .map(|(label, body, detail)| CompletionCandidate {
            label: (*label).to_string(),
            insert_text: (*body).to_string(),
            detail: (*detail).to_string(),
            is_snippet: true,
        })
        .collect()
}
