use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const LSP_RESPONSE_TIMEOUT: Duration = Duration::from_millis(2500);

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

struct LspSession {
    child: Child,
    stdin: ChildStdin,
    msg_rx: mpsc::Receiver<Value>,
    next_id: u64,
    initialized: bool,
    opened_docs: HashSet<String>,
    doc_versions: HashMap<String, i32>,
    language_id: &'static str,
}

static SESSION_POOL: OnceLock<Mutex<HashMap<String, LspSession>>> = OnceLock::new();

fn session_pool() -> &'static Mutex<HashMap<String, LspSession>> {
    SESSION_POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn collect_snapshot(
    path: &Path,
    text: &str,
    line: usize,
    col: usize,
    request: RequestKind,
) -> Snapshot {
    let Some(server) = resolve_server(path) else {
        return fallback_snapshot(path);
    };

    let key = session_key(path, server.language_id);
    let mut pool = match session_pool().lock() {
        Ok(guard) => guard,
        Err(_) => return fallback_snapshot(path),
    };

    if !pool.contains_key(&key) {
        let Ok(session) = LspSession::start(&server) else {
            return fallback_snapshot(path);
        };
        pool.insert(key.clone(), session);
    }

    let result = pool
        .get_mut(&key)
        .and_then(|session| session.collect(path, text, line, col, request).ok());
    if let Some(snapshot) = result {
        snapshot
    } else {
        pool.remove(&key);
        fallback_snapshot(path)
    }
}

impl LspSession {
    fn start(server: &ServerSpec) -> Result<Self, String> {
        let mut child = Command::new(&server.program)
            .args(&server.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "LSP stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "LSP stdout unavailable".to_string())?;
        let (tx, rx) = mpsc::channel();
        spawn_stdout_reader(stdout, tx);

        Ok(Self {
            child,
            stdin,
            msg_rx: rx,
            next_id: 1,
            initialized: false,
            opened_docs: HashSet::new(),
            doc_versions: HashMap::new(),
            language_id: server.language_id,
        })
    }

    fn collect(
        &mut self,
        path: &Path,
        text: &str,
        line: usize,
        col: usize,
        request: RequestKind,
    ) -> Result<Snapshot, String> {
        if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("LSP process exited".to_string());
        }

        let mut snapshot = Snapshot {
            completions: snippets_for(path),
            diagnostics: Vec::new(),
            formatted_text: None,
            definitions: Vec::new(),
            references: Vec::new(),
            implementations: Vec::new(),
            had_server: true,
        };

        if !self.initialized {
            let init_id = self.next_id;
            self.next_id += 1;
            write_lsp_message(
                &mut self.stdin,
                &request_msg(
                    init_id,
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
                ),
            )
            .map_err(|e| e.to_string())?;
            self.wait_for_pending(&mut HashSet::from([init_id]), &mut snapshot, text)?;
            write_lsp_message(&mut self.stdin, &notification_msg("initialized", json!({})))
                .map_err(|e| e.to_string())?;
            self.initialized = true;
        }

        let uri = format!("file://{}", path.to_string_lossy());
        let version = self.doc_versions.entry(uri.clone()).or_insert(0);
        *version += 1;

        if self.opened_docs.contains(&uri) {
            write_lsp_message(
                &mut self.stdin,
                &notification_msg(
                    "textDocument/didChange",
                    json!({
                        "textDocument": {"uri": uri, "version": *version},
                        "contentChanges": [{"text": text}],
                    }),
                ),
            )
            .map_err(|e| e.to_string())?;
        } else {
            write_lsp_message(
                &mut self.stdin,
                &notification_msg(
                    "textDocument/didOpen",
                    json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": self.language_id,
                            "version": *version,
                            "text": text,
                        }
                    }),
                ),
            )
            .map_err(|e| e.to_string())?;
            self.opened_docs.insert(uri.clone());
        }

        let mut pending = HashSet::new();
        let mut completion_id = None;
        let mut formatting_id = None;
        let mut definition_id = None;
        let mut references_id = None;
        let mut implementations_id = None;

        if request.want_completion {
            let req_id = self.next_id;
            self.next_id += 1;
            completion_id = Some(req_id);
            pending.insert(req_id);
            write_lsp_message(
                &mut self.stdin,
                &request_msg(
                    req_id,
                    "textDocument/completion",
                    json!({
                        "textDocument": {"uri": uri},
                        "position": {"line": line, "character": col},
                    }),
                ),
            )
            .map_err(|e| e.to_string())?;
        }

        if request.want_formatting {
            let req_id = self.next_id;
            self.next_id += 1;
            formatting_id = Some(req_id);
            pending.insert(req_id);
            write_lsp_message(
                &mut self.stdin,
                &request_msg(
                    req_id,
                    "textDocument/formatting",
                    json!({
                        "textDocument": {"uri": uri},
                        "options": {
                            "tabSize": 4,
                            "insertSpaces": true,
                            "trimTrailingWhitespace": true,
                            "insertFinalNewline": false,
                        },
                    }),
                ),
            )
            .map_err(|e| e.to_string())?;
        }

        if request.want_definition {
            let req_id = self.next_id;
            self.next_id += 1;
            definition_id = Some(req_id);
            pending.insert(req_id);
            write_lsp_message(
                &mut self.stdin,
                &request_msg(
                    req_id,
                    "textDocument/definition",
                    json!({
                        "textDocument": {"uri": uri},
                        "position": {"line": line, "character": col},
                    }),
                ),
            )
            .map_err(|e| e.to_string())?;
        }

        if request.want_references {
            let req_id = self.next_id;
            self.next_id += 1;
            references_id = Some(req_id);
            pending.insert(req_id);
            write_lsp_message(
                &mut self.stdin,
                &request_msg(
                    req_id,
                    "textDocument/references",
                    json!({
                        "textDocument": {"uri": uri},
                        "position": {"line": line, "character": col},
                        "context": {"includeDeclaration": true},
                    }),
                ),
            )
            .map_err(|e| e.to_string())?;
        }

        if request.want_implementations {
            let req_id = self.next_id;
            self.next_id += 1;
            implementations_id = Some(req_id);
            pending.insert(req_id);
            write_lsp_message(
                &mut self.stdin,
                &request_msg(
                    req_id,
                    "textDocument/implementation",
                    json!({
                        "textDocument": {"uri": uri},
                        "position": {"line": line, "character": col},
                    }),
                ),
            )
            .map_err(|e| e.to_string())?;
        }

        if !pending.is_empty() {
            let deadline = Instant::now() + LSP_RESPONSE_TIMEOUT;
            while !pending.is_empty() {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                let timeout = deadline.saturating_duration_since(now);
                let message = match self.msg_rx.recv_timeout(timeout) {
                    Ok(msg) => msg,
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err("LSP reader disconnected".to_string())
                    }
                };
                handle_message(&message, &mut snapshot);
                if let Some(resp_id) = message.get("id").and_then(|v| v.as_u64()) {
                    if pending.remove(&resp_id) {
                        if Some(resp_id) == completion_id {
                            if let Some(items) = completion_items_from_result(message.get("result"))
                            {
                                snapshot.completions.extend(items);
                            }
                        } else if Some(resp_id) == formatting_id {
                            snapshot.formatted_text =
                                formatting_from_result(message.get("result"), text);
                        } else if Some(resp_id) == definition_id {
                            snapshot.definitions = locations_from_result(message.get("result"));
                        } else if Some(resp_id) == references_id {
                            snapshot.references = locations_from_result(message.get("result"));
                        } else if Some(resp_id) == implementations_id {
                            snapshot.implementations = locations_from_result(message.get("result"));
                        }
                    }
                }
            }
        }

        Ok(snapshot)
    }

    fn wait_for_pending(
        &mut self,
        pending: &mut HashSet<u64>,
        snapshot: &mut Snapshot,
        _text: &str,
    ) -> Result<(), String> {
        let deadline = Instant::now() + LSP_RESPONSE_TIMEOUT;
        while !pending.is_empty() {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let timeout = deadline.saturating_duration_since(now);
            let message = match self.msg_rx.recv_timeout(timeout) {
                Ok(msg) => msg,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("LSP reader disconnected".to_string())
                }
            };
            handle_message(&message, snapshot);
            if let Some(resp_id) = message.get("id").and_then(|v| v.as_u64()) {
                pending.remove(&resp_id);
            }
        }
        Ok(())
    }
}

fn spawn_stdout_reader(stdout: ChildStdout, tx: mpsc::Sender<Value>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        while let Some(msg) = read_lsp_message(&mut reader) {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
}

fn read_lsp_message(reader: &mut BufReader<ChildStdout>) -> Option<Value> {
    let mut content_len = None;
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).ok()?;
        if read == 0 {
            return None;
        }
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_len = value.trim().parse::<usize>().ok();
        }
    }

    let len = content_len?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn session_key(path: &Path, language_id: &str) -> String {
    let workspace = workspace_scope_for(path);
    format!("{}::{language_id}", workspace.to_string_lossy())
}

fn workspace_scope_for(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if ancestor.join(".git").exists() {
            return ancestor.to_path_buf();
        }
    }
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn handle_message(message: &Value, snapshot: &mut Snapshot) {
    if let Some(method) = message.get("method").and_then(|v| v.as_str()) {
        if method == "textDocument/publishDiagnostics" {
            snapshot.diagnostics.clear();
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
                    let severity = diag.get("severity").and_then(|v| v.as_u64()).unwrap_or(3) as u8;
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
    }
}

fn fallback_snapshot(path: &Path) -> Snapshot {
    Snapshot {
        completions: snippets_for(path),
        diagnostics: Vec::new(),
        formatted_text: None,
        definitions: Vec::new(),
        references: Vec::new(),
        implementations: Vec::new(),
        had_server: false,
    }
}

fn write_lsp_message(mut writer: impl Write, body: &Value) -> std::io::Result<()> {
    let encoded = body.to_string();
    write!(
        writer,
        "Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}",
        encoded.len(),
        encoded
    )?;
    writer.flush()
}

#[cfg(test)]
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

    let mut parsed = Vec::new();
    for edit in edits {
        let range = edit.get("range")?;
        let start_line = range
            .get("start")
            .and_then(|v| v.get("line"))
            .and_then(|v| v.as_u64())? as usize;
        let start_char = range
            .get("start")
            .and_then(|v| v.get("character"))
            .and_then(|v| v.as_u64())? as usize;
        let end_line = range
            .get("end")
            .and_then(|v| v.get("line"))
            .and_then(|v| v.as_u64())? as usize;
        let end_char = range
            .get("end")
            .and_then(|v| v.get("character"))
            .and_then(|v| v.as_u64())? as usize;
        let new_text = edit
            .get("newText")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let start = lsp_position_to_char_idx(original, start_line, start_char);
        let end = lsp_position_to_char_idx(original, end_line, end_char);
        if start > end {
            return None;
        }
        parsed.push((start, end, new_text));
    }

    parsed.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    let mut formatted = original.to_string();
    for (start, end, new_text) in parsed {
        let start_byte = char_to_byte_idx(&formatted, start);
        let end_byte = char_to_byte_idx(&formatted, end);
        if start_byte > end_byte || end_byte > formatted.len() {
            return None;
        }
        formatted.replace_range(start_byte..end_byte, &new_text);
    }
    if formatted == original {
        None
    } else {
        Some(formatted)
    }
}

fn lsp_position_to_char_idx(text: &str, line: usize, character: usize) -> usize {
    let mut current_line = 0usize;
    let mut current_char = 0usize;
    let mut line_start_char = 0usize;

    for ch in text.chars() {
        if current_line == line {
            break;
        }
        current_char += 1;
        if ch == '\n' {
            current_line += 1;
            line_start_char = current_char;
        }
    }

    let target_line = current_line == line;
    if !target_line {
        return text.chars().count();
    }

    let mut line_len = 0usize;
    for ch in text.chars().skip(line_start_char) {
        if ch == '\n' {
            break;
        }
        line_len += 1;
    }
    line_start_char + character.min(line_len)
}

fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    match text.char_indices().nth(char_idx) {
        Some((byte_idx, _)) => byte_idx,
        None => text.len(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_applies_multiple_text_edits() {
        let original = "let a = 1;\nlet b = 2;\n";
        let result = serde_json::json!([
            {
                "range": {
                    "start": {"line": 0, "character": 4},
                    "end": {"line": 0, "character": 5}
                },
                "newText": "alpha"
            },
            {
                "range": {
                    "start": {"line": 1, "character": 4},
                    "end": {"line": 1, "character": 5}
                },
                "newText": "beta"
            }
        ]);

        let formatted = formatting_from_result(Some(&result), original).unwrap();
        assert_eq!(formatted, "let alpha = 1;\nlet beta = 2;\n");
    }

    #[test]
    fn parse_lsp_stream_parses_multiple_messages() {
        let first = serde_json::json!({"jsonrpc":"2.0","id":1,"result":"ok"}).to_string();
        let second = serde_json::json!({"jsonrpc":"2.0","id":2,"result":"done"}).to_string();
        let payload = format!(
            "Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            first.len(),
            first,
            second.len(),
            second
        );
        let parsed = parse_lsp_stream(payload.as_bytes());
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].get("id").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(parsed[1].get("id").and_then(|v| v.as_u64()), Some(2));
    }

    #[test]
    fn lsp_position_to_char_idx_handles_bounds() {
        let text = "ab\ncd";
        assert_eq!(lsp_position_to_char_idx(text, 0, 1), 1);
        assert_eq!(lsp_position_to_char_idx(text, 1, 1), 4);
        assert_eq!(lsp_position_to_char_idx(text, 9, 1), text.chars().count());
    }
}
