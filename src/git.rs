use std::path::{Path, PathBuf};
use std::process::Command;

use crate::editor::{Editor, InlineBlameEntry};

#[derive(Clone, Debug, Default)]
pub struct GitChangedFile {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Clone, Debug, Default)]
pub struct GitCommitEntry {
    pub hash: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default)]
pub struct GitPanelState {
    pub files: Vec<GitChangedFile>,
    pub commits: Vec<GitCommitEntry>,
    pub selected_file: Option<String>,
    pub diff_text: String,
    pub blame_text: String,
    pub commit_message: String,
    pub branch_input: String,
    pub stash_message: String,
    pub op_status: String,
    pub last_refresh: f64,
}

pub fn resolve_git_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

pub fn read_git_files(repo: &Path) -> Vec<GitChangedFile> {
    let output = match Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(repo)
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.len() < 4 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].to_string();
        let status = format!("{}{}", index_status, worktree_status);
        out.push(GitChangedFile {
            path,
            status,
            staged: index_status != ' ',
        });
    }
    out
}

pub fn read_git_commits(repo: &Path) -> Vec<GitCommitEntry> {
    let output = match Command::new("git")
        .arg("log")
        .arg("-n")
        .arg("20")
        .arg("--pretty=format:%h\t%s")
        .current_dir(repo)
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (hash, summary) = line.split_once('\t')?;
            Some(GitCommitEntry {
                hash: hash.to_string(),
                summary: summary.to_string(),
            })
        })
        .collect()
}

pub fn read_git_diff_for_file(repo: &Path, file: &str) -> String {
    let output = Command::new("git")
        .arg("diff")
        .arg("--")
        .arg(file)
        .current_dir(repo)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        _ => String::new(),
    }
}

pub fn read_active_line_blame(repo: &Path, editor: &Editor) -> String {
    let Some(path) = editor.file_path.as_ref() else {
        return "No file".to_string();
    };
    let Ok(relative) = path.strip_prefix(repo) else {
        return "Not in repo".to_string();
    };
    let line = editor.cursors.first().map(|c| c.pos.line + 1).unwrap_or(1);
    let output = Command::new("git")
        .arg("blame")
        .arg("-L")
        .arg(format!("{line},{line}"))
        .arg("--")
        .arg(relative)
        .current_dir(repo)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "Blame unavailable".to_string(),
    }
}

pub fn git_stage_file(repo: &Path, file: &str) -> bool {
    Command::new("git")
        .arg("add")
        .arg("--")
        .arg(file)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn git_unstage_file(repo: &Path, file: &str) -> bool {
    Command::new("git")
        .arg("reset")
        .arg("HEAD")
        .arg("--")
        .arg(file)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn git_commit(repo: &Path, message: &str) -> bool {
    Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg(message)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn git_checkout_branch(repo: &Path, branch: &str) -> bool {
    if branch.is_empty() {
        return false;
    }
    Command::new("git")
        .arg("checkout")
        .arg(branch)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn git_merge_branch(repo: &Path, branch: &str) -> bool {
    if branch.is_empty() {
        return false;
    }
    Command::new("git")
        .arg("merge")
        .arg(branch)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn git_rebase_branch(repo: &Path, branch: &str) -> bool {
    if branch.is_empty() {
        return false;
    }
    Command::new("git")
        .arg("rebase")
        .arg(branch)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn git_stash_push(repo: &Path, message: &str) -> bool {
    let mut cmd = Command::new("git");
    cmd.arg("stash").arg("push");
    if !message.is_empty() {
        cmd.arg("-m").arg(message);
    }
    cmd.current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn git_stash_pop(repo: &Path) -> bool {
    Command::new("git")
        .arg("stash")
        .arg("pop")
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn read_git_info(cwd: Option<&Path>) -> Option<crate::ui::status_bar::GitInfo> {
    let cwd = cwd?;

    let mut rev_cmd = Command::new("git");
    rev_cmd
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(cwd);
    let rev_output = rev_cmd.output().ok()?;
    if !rev_output.status.success() {
        return None;
    }
    let toplevel = String::from_utf8_lossy(&rev_output.stdout)
        .trim()
        .to_string();
    if toplevel.is_empty() {
        return None;
    }
    let toplevel = Path::new(&toplevel);
    if !cwd.starts_with(toplevel) {
        return None;
    }

    let mut cmd = Command::new("git");
    cmd.arg("status").arg("-sb");
    cmd.current_dir(cwd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let first = lines.next()?.trim();
    if !first.starts_with("## ") {
        return None;
    }
    let mut branch = first.trim_start_matches("## ").to_string();
    let mut ahead = 0usize;
    let mut behind = 0usize;
    if let Some((name, rest)) = branch.clone().split_once("...") {
        branch = name.to_string();
        if let Some(start) = rest.find('[') {
            if let Some(end) = rest.find(']') {
                let stats = &rest[start + 1..end];
                for part in stats.split(',') {
                    let part = part.trim();
                    if let Some(value) = part.strip_prefix("ahead ") {
                        ahead = value.parse().unwrap_or(0);
                    } else if let Some(value) = part.strip_prefix("behind ") {
                        behind = value.parse().unwrap_or(0);
                    }
                }
            }
        }
    }
    let dirty = lines.next().is_some();
    Some(crate::ui::status_bar::GitInfo {
        branch,
        ahead,
        behind,
        dirty,
    })
}

pub fn read_inline_blame(path: &Path) -> Vec<InlineBlameEntry> {
    let cwd = match path.parent() {
        Some(parent) => parent,
        None => return Vec::new(),
    };

    let mut cmd = Command::new("git");
    cmd.arg("blame")
        .arg("--line-porcelain")
        .arg("--")
        .arg(path)
        .current_dir(cwd);
    let output = match cmd.output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let mut lines = Vec::new();
    let mut commit_short = String::new();
    let mut author = String::new();
    let mut summary = String::new();

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.starts_with('\t') {
            lines.push(InlineBlameEntry {
                commit_short: commit_short.clone(),
                author: author.clone(),
                summary: summary.clone(),
            });
            continue;
        }

        if let Some((head, _)) = line.split_once(' ') {
            if head.len() >= 8 && head.chars().all(|c| c.is_ascii_hexdigit()) {
                commit_short = if head.starts_with("00000000") {
                    "working".to_string()
                } else {
                    head.chars().take(8).collect()
                };
                author.clear();
                summary.clear();
                continue;
            }
        }

        if let Some(value) = line.strip_prefix("author ") {
            author = value.to_string();
            continue;
        }
        if let Some(value) = line.strip_prefix("summary ") {
            summary = value.to_string();
        }
    }

    lines
}
