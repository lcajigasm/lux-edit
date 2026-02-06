use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub commands: Vec<PluginCommand>,
    pub syntax_packages: Vec<String>,
    pub formatters: Vec<String>,
    pub tasks: Vec<String>,
    pub keymaps: Vec<String>,
    pub scripts: Vec<String>,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
    pub shortcut: String,
}

#[derive(Clone, Debug, Default)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub source_manifest: String,
}

pub fn load_plugin_manifests(workspace: &Path) -> Vec<PluginManifest> {
    let plugins_dir = workspace.join(".lux").join("plugins");
    let Ok(entries) = std::fs::read_dir(&plugins_dir) else {
        return Vec::new();
    };
    let mut manifests = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                let manifest = parse_manifest(value, path.clone());
                if !manifest.id.is_empty() {
                    manifests.push(manifest);
                }
            }
        }
    }
    manifests
}

fn parse_manifest(value: serde_json::Value, path: PathBuf) -> PluginManifest {
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&id)
        .to_string();
    let commands = value
        .get("commands")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| PluginCommand {
                    id: v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    title: v
                        .get("title")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    shortcut: v
                        .get("shortcut")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .filter(|c| !c.id.is_empty() && !c.title.is_empty())
                .collect()
        })
        .unwrap_or_default();
    PluginManifest {
        id,
        name,
        commands,
        syntax_packages: read_string_array(&value, "syntax_packages"),
        formatters: read_string_array(&value, "formatters"),
        tasks: read_string_array(&value, "tasks"),
        keymaps: read_string_array(&value, "keymaps"),
        scripts: read_string_array(&value, "scripts"),
        path,
    }
}

fn read_string_array(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn load_registry(workspace: &Path) -> Vec<RegistryEntry> {
    let path = workspace.join(".lux").join("registry.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    value
        .get("plugins")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|p| RegistryEntry {
                    id: p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: p
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    source_manifest: p
                        .get("source_manifest")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .filter(|p| !p.id.is_empty() && !p.source_manifest.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn install_or_update_registry_plugin(workspace: &Path, entry: &RegistryEntry) -> Result<(), String> {
    let source = PathBuf::from(&entry.source_manifest);
    if !source.exists() {
        return Err("source manifest not found".to_string());
    }
    let target_dir = workspace.join(".lux").join("plugins");
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let target = target_dir.join(format!("{}.json", entry.id));
    std::fs::copy(source, target).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn run_plugin_command(
    plugin: &PluginManifest,
    command_id: &str,
    sandboxed: bool,
) -> Result<String, String> {
    let Some(base_dir) = plugin.path.parent() else {
        return Err("Invalid plugin path".to_string());
    };
    let script = resolve_script_for_command(base_dir, command_id)
        .ok_or_else(|| format!("No script found for command: {command_id}"))?;
    run_lifecycle_hook(base_dir, "pre", sandboxed).ok();
    let output = run_script(script.as_path(), base_dir, sandboxed)?;
    run_lifecycle_hook(base_dir, "post", sandboxed).ok();
    Ok(output)
}

fn resolve_script_for_command(base_dir: &Path, command_id: &str) -> Option<PathBuf> {
    for ext in ["lua", "js", "rs", "sh"] {
        let candidate = base_dir.join(format!("{command_id}.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn run_lifecycle_hook(base_dir: &Path, hook: &str, sandboxed: bool) -> Result<(), String> {
    let hook_script = base_dir.join("hooks").join(format!("{hook}.sh"));
    if !hook_script.exists() {
        return Ok(());
    }
    run_script(hook_script.as_path(), base_dir, sandboxed).map(|_| ())
}

fn run_script(script: &Path, cwd: &Path, sandboxed: bool) -> Result<String, String> {
    let ext = script
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut cmd = match ext.as_str() {
        "lua" => {
            let mut c = Command::new("lua");
            c.arg(script);
            c
        }
        "js" => {
            let mut c = Command::new("node");
            c.arg(script);
            c
        }
        "rs" => {
            let mut c = Command::new("rust-script");
            c.arg(script);
            c
        }
        _ => {
            let mut c = Command::new("sh");
            c.arg(script);
            c
        }
    };
    cmd.current_dir(cwd);
    if sandboxed {
        cmd.env("LUX_SANDBOX", "1");
    }
    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
