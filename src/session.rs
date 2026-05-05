use std::path::PathBuf;

use crate::editor::Editor;

pub fn session_file_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".lux")
        .join("session.json")
}

pub fn recovery_dir_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".lux")
        .join("recovery")
}

pub fn persist_session_snapshot(editors: &[Editor], active_tab: usize) {
    let path = session_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let files: Vec<String> = editors
        .iter()
        .filter_map(|e| e.file_path.as_ref())
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let json = serde_json::json!({
        "files": files,
        "active_tab": active_tab,
    });
    let _ = std::fs::write(path, json.to_string());
}

pub fn load_session_editors() -> Option<(Vec<Editor>, usize)> {
    let path = session_file_path();
    let raw = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let files = value.get("files")?.as_array()?;
    let mut editors = Vec::new();
    for file in files {
        let Some(path_str) = file.as_str() else {
            continue;
        };
        if let Ok(editor) = Editor::from_file(PathBuf::from(path_str)) {
            editors.push(editor);
        }
    }
    if editors.is_empty() {
        return None;
    }
    let active = value
        .get("active_tab")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
        .min(editors.len().saturating_sub(1));
    Some((editors, active))
}

pub fn persist_recovery_snapshot(editor: &Editor) -> std::io::Result<()> {
    let dir = recovery_dir_path();
    std::fs::create_dir_all(&dir)?;
    let name = format!(
        "untitled-{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    std::fs::write(dir.join(name), editor.rope.to_string())
}

pub fn recent_workspaces_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".lux")
        .join("recent_workspaces.json")
}

pub fn load_recent_workspaces() -> Vec<PathBuf> {
    let path = recent_workspaces_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    value
        .get("workspaces")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn persist_recent_workspaces(workspaces: &[PathBuf]) {
    let path = recent_workspaces_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let list: Vec<String> = workspaces
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let json = serde_json::json!({ "workspaces": list });
    let _ = std::fs::write(path, json.to_string());
}
