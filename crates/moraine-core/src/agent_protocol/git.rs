use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

/// Mechanically captured Git facts. Never agent-authored prose.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitContextSummary {
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detached: Option<bool>,
    /// clean | dirty | unknown
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_tree: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_file_count: Option<usize>,
}

const MAX_CHANGED_FILES: usize = 20;

/// Capture Git context for `cwd` when a repository is present.
pub fn capture_git_context(cwd: &Path) -> GitContextSummary {
    let root = git_stdout(cwd, &["rev-parse", "--show-toplevel"]);
    let Some(root) = root else {
        return GitContextSummary {
            available: false,
            ..Default::default()
        };
    };
    let root = root.trim().to_string();
    let head = git_stdout(cwd, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string());
    let abbrev = git_stdout(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let detached = abbrev == "HEAD";
    let branch = if detached || abbrev.is_empty() {
        None
    } else {
        Some(abbrev)
    };
    let upstream = git_stdout(
        cwd,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .map(|s| s.trim().to_string());
    let status = git_stdout(cwd, &["status", "--porcelain"]);
    let (working_tree, changed_files, changed_file_count) = match status {
        None => (Some("unknown".into()), Vec::new(), None),
        Some(s) if s.trim().is_empty() => (Some("clean".into()), Vec::new(), Some(0)),
        Some(s) => {
            let mut files: Vec<String> = s
                .lines()
                .filter(|l| !l.is_empty())
                .map(porcelain_path)
                .filter(|p| !p.is_empty())
                .collect();
            let count = files.len();
            if files.len() > MAX_CHANGED_FILES {
                files.truncate(MAX_CHANGED_FILES);
                files.push(format!("…and {} more", count - MAX_CHANGED_FILES));
            }
            (Some("dirty".into()), files, Some(count))
        }
    };

    GitContextSummary {
        available: true,
        repository_root: Some(root),
        branch,
        head,
        upstream,
        detached: Some(detached),
        working_tree,
        changed_files,
        changed_file_count,
    }
}

/// Parse a single `git status --porcelain` line into a path.
///
/// Porcelain lines are `XY<space>PATH` (or rename `XY<space>ORIG -> PATH`).
/// Never trim the line before slicing: a leading space in `XY` is meaningful, and
/// trimming it shifts the path slice left by one character.
fn porcelain_path(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() >= 3 && bytes[2] == b' ' {
        let path = &line[3..];
        if let Some((_, newer)) = path.split_once(" -> ") {
            newer.trim_end().to_string()
        } else {
            // Paths may start with `.`; do not trim_start.
            path.trim_end().to_string()
        }
    } else {
        line.trim().to_string()
    }
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::porcelain_path;

    #[test]
    fn porcelain_keeps_dotfiles_and_first_char() {
        assert_eq!(porcelain_path(" M ARCHITECTURE.md"), "ARCHITECTURE.md");
        assert_eq!(
            porcelain_path("?? .github/workflows/ci.yml"),
            ".github/workflows/ci.yml"
        );
        assert_eq!(porcelain_path("M  Cargo.toml"), "Cargo.toml");
        assert_eq!(porcelain_path("R  old.md -> new.md"), "new.md");
    }
}
