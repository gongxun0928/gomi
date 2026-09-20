use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub is_primary: bool,
}

pub fn has_git(path: &Path) -> bool {
    path.join(".git").exists()
}

pub fn is_primary_checkout(path: &Path) -> bool {
    let git = path.join(".git");
    git.is_dir()
}

pub fn is_linked_worktree(path: &Path) -> bool {
    let git = path.join(".git");
    git.is_file()
}

pub fn is_checkout(path: &Path) -> bool {
    has_git(path)
}

pub fn worktree_list(repo: &Path) -> Vec<Worktree> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "worktree", "list", "--porcelain"])
        .output();
    let Ok(out) = out else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut list = Vec::new();
    let mut current: Option<PathBuf> = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            current = Some(PathBuf::from(p));
        } else if line.is_empty() {
            if let Some(path) = current.take() {
                let is_primary = path.join(".git").is_dir();
                list.push(Worktree { path, is_primary });
            }
        }
    }
    if let Some(path) = current {
        let is_primary = path.join(".git").is_dir();
        list.push(Worktree { path, is_primary });
    }
    list
}

pub fn is_dirty(path: &Path) -> bool {
    let out = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "status", "--porcelain"])
        .output();
    match out {
        Ok(o) if o.status.success() => !o.stdout.is_empty(),
        _ => false,
    }
}

pub fn head_mtime(path: &Path) -> Option<SystemTime> {
    let git = path.join(".git");
    let head = if git.is_dir() {
        git.join("HEAD")
    } else if git.is_file() {
        // gitdir: /path/to/git/worktrees/name
        let text = std::fs::read_to_string(&git).ok()?;
        let dir = text
            .lines()
            .find_map(|l| l.strip_prefix("gitdir:"))
            .map(str::trim)
            .map(PathBuf::from)?;
        dir.join("HEAD")
    } else {
        return None;
    };
    std::fs::metadata(head)
        .and_then(|m| m.modified())
        .or_else(|_| std::fs::metadata(path).and_then(|m| m.modified()))
        .ok()
}

pub fn dir_mtime(path: &Path) -> Option<SystemTime> {
    let a = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let b = head_mtime(path);
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

pub fn nearest_git_root(start: &Path, stop: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join(".git").exists() {
            return Some(cur);
        }
        if cur == stop {
            return None;
        }
        if !cur.pop() {
            return None;
        }
        if stop != Path::new("/") && !cur.starts_with(stop) && cur != stop {
            return None;
        }
    }
}
