use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

fn git(path: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd
}

use crate::inventory::{CleanMethod, DefaultAction};

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub locked: bool,
    pub prunable: bool,
    pub is_primary: bool,
}

#[derive(Debug, Clone)]
pub struct CheckoutFacts {
    pub is_primary: bool,
    pub is_linked: bool,
    pub locked: bool,
    pub dirty: Option<bool>,
    pub detached: bool,
    pub branch: Option<String>,
    pub unique_vs_main: Option<u64>,
    pub stale: bool,
    pub mtime: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeVerdict {
    KeepPrimary,
    KeepUnknown { why: &'static str },
    KeepLocked,
    KeepDirty,
    KeepUniqueCommits { count: u64 },
    KeepRecent,
    SuggestRemove,
    SuggestTrashCopy,
}

impl WorktreeVerdict {
    pub fn action(&self) -> (DefaultAction, CleanMethod) {
        match self {
            WorktreeVerdict::KeepPrimary => (DefaultAction::None, CleanMethod::None),
            WorktreeVerdict::KeepUnknown { .. }
            | WorktreeVerdict::KeepLocked
            | WorktreeVerdict::KeepDirty
            | WorktreeVerdict::KeepUniqueCommits { .. }
            | WorktreeVerdict::KeepRecent => (DefaultAction::WarnOnly, CleanMethod::None),
            WorktreeVerdict::SuggestRemove => {
                (DefaultAction::SuggestTrash, CleanMethod::GitWorktreeRemove)
            }
            WorktreeVerdict::SuggestTrashCopy => {
                (DefaultAction::SuggestTrash, CleanMethod::Trash)
            }
        }
    }

    pub fn reason(&self) -> String {
        match self {
            WorktreeVerdict::KeepPrimary => {
                "primary checkout; never deletable".into()
            }
            WorktreeVerdict::KeepUnknown { why } => {
                format!("git safety unknown ({why}); not suggesting delete")
            }
            WorktreeVerdict::KeepLocked => "worktree is locked; not safe to remove".into(),
            WorktreeVerdict::KeepDirty => "uncommitted changes; not safe to remove".into(),
            WorktreeVerdict::KeepUniqueCommits { count } => {
                format!("{count} commit(s) not on main/master; not safe to remove")
            }
            WorktreeVerdict::KeepRecent => {
                "working tree clean, but not stale yet; wait".into()
            }
            WorktreeVerdict::SuggestRemove => {
                "linked worktree, clean, no unique commits vs main, stale; safe to git worktree remove"
                    .into()
            }
            WorktreeVerdict::SuggestTrashCopy => {
                "copy/clone, clean, no unique commits vs main, stale; safe to Trash".into()
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            WorktreeVerdict::KeepPrimary => "keep: primary",
            WorktreeVerdict::KeepUnknown { .. } => "keep: unknown",
            WorktreeVerdict::KeepLocked => "keep: locked",
            WorktreeVerdict::KeepDirty => "keep: dirty",
            WorktreeVerdict::KeepUniqueCommits { .. } => "keep: unique commits",
            WorktreeVerdict::KeepRecent => "keep: recent",
            WorktreeVerdict::SuggestRemove => "suggest: worktree remove",
            WorktreeVerdict::SuggestTrashCopy => "suggest: trash",
        }
    }
}

pub fn classify(facts: &CheckoutFacts) -> WorktreeVerdict {
    if facts.is_primary {
        return WorktreeVerdict::KeepPrimary;
    }
    if facts.dirty.is_none() {
        return WorktreeVerdict::KeepUnknown {
            why: "git status failed",
        };
    }
    if facts.locked {
        return WorktreeVerdict::KeepLocked;
    }
    if facts.dirty == Some(true) {
        return WorktreeVerdict::KeepDirty;
    }
    match facts.unique_vs_main {
        None => {
            return WorktreeVerdict::KeepUnknown {
                why: "no main/master ref to compare",
            };
        }
        Some(0) => {}
        Some(count) => {
            return WorktreeVerdict::KeepUniqueCommits { count };
        }
    }
    if !facts.stale {
        return WorktreeVerdict::KeepRecent;
    }
    if facts.is_linked {
        WorktreeVerdict::SuggestRemove
    } else {
        WorktreeVerdict::SuggestTrashCopy
    }
}

pub fn inspect_checkout(path: &Path, stale_after_days: u64, agent_copy: bool) -> CheckoutFacts {
    let is_linked = is_linked_worktree(path);
    let is_primary = is_primary_checkout(path) && !agent_copy;
    let listed = worktree_list(path)
        .into_iter()
        .find(|w| same_path(&w.path, path));
    let mtime = dir_mtime(path);
    CheckoutFacts {
        is_primary,
        is_linked,
        locked: listed.as_ref().map(|w| w.locked).unwrap_or(false),
        dirty: is_dirty(path),
        detached: listed.as_ref().map(|w| w.detached).unwrap_or(false),
        branch: listed
            .as_ref()
            .and_then(|w| w.branch.clone())
            .or_else(|| current_branch(path)),
        unique_vs_main: if is_primary {
            Some(0)
        } else {
            unique_commits_vs_main(path)
        },
        stale: is_stale(mtime, stale_after_days),
        mtime,
    }
}

pub fn has_git(path: &Path) -> bool {
    path.join(".git").exists()
}

pub fn is_primary_checkout(path: &Path) -> bool {
    path.join(".git").is_dir()
}

pub fn is_linked_worktree(path: &Path) -> bool {
    path.join(".git").is_file()
}

pub fn is_checkout(path: &Path) -> bool {
    has_git(path)
}

/// Unregister and delete a linked worktree. Never falls back to `rm`.
pub fn worktree_remove(path: &Path) -> std::result::Result<(), String> {
    let out = git(path)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("not a git worktree".into());
    }
    let common = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if common.is_empty() {
        return Err("empty git-common-dir".into());
    }
    let common_path = {
        let p = PathBuf::from(&common);
        if p.is_absolute() {
            p
        } else {
            path.join(p)
        }
    };
    let out = Command::new("git")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--git-dir")
        .arg(&common_path)
        .args(["worktree", "remove"])
        .arg(path)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            Err("git worktree remove failed".into())
        } else {
            Err(err)
        }
    }
}

pub fn worktree_list(repo: &Path) -> Vec<Worktree> {
    let out = git(repo).args(["worktree", "list", "--porcelain"]).output();
    let Ok(out) = out else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_worktree_porcelain(&String::from_utf8_lossy(&out.stdout))
}

pub fn parse_worktree_porcelain(text: &str) -> Vec<Worktree> {
    let mut list = Vec::new();
    let mut current: Option<Worktree> = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(prev) = current.take() {
                list.push(finish_worktree(prev));
            }
            current = Some(Worktree {
                path: PathBuf::from(p),
                head: None,
                branch: None,
                detached: false,
                locked: false,
                prunable: false,
                is_primary: false,
            });
            continue;
        }
        let Some(wt) = current.as_mut() else {
            continue;
        };
        if let Some(head) = line.strip_prefix("HEAD ") {
            wt.head = Some(head.trim().to_string());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            wt.branch = short_branch(branch.trim());
        } else if line == "detached" || line.starts_with("detached ") {
            wt.detached = true;
        } else if line == "locked" || line.starts_with("locked ") {
            wt.locked = true;
        } else if line == "prunable" || line.starts_with("prunable ") {
            wt.prunable = true;
        } else if line.is_empty() {
            list.push(finish_worktree(current.take().unwrap()));
        }
    }
    if let Some(wt) = current {
        list.push(finish_worktree(wt));
    }
    list
}

fn finish_worktree(mut wt: Worktree) -> Worktree {
    wt.is_primary = wt.path.join(".git").is_dir();
    wt
}

fn short_branch(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    Some(
        raw.strip_prefix("refs/heads/")
            .unwrap_or(raw)
            .to_string(),
    )
}

pub fn is_dirty(path: &Path) -> Option<bool> {
    let out = git(path).args(["status", "--porcelain"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(!out.stdout.is_empty())
}

pub fn unique_commits_vs_main(path: &Path) -> Option<u64> {
    let mut not_refs = Vec::new();
    for r in [
        "refs/heads/main",
        "refs/heads/master",
        "refs/remotes/origin/main",
        "refs/remotes/origin/master",
    ] {
        if rev_exists(path, r) {
            not_refs.push(r);
        }
    }
    if not_refs.is_empty() {
        return None;
    }
    let mut args = vec![
        "rev-list".to_string(),
        "--count".into(),
        "HEAD".into(),
        "--not".into(),
    ];
    args.extend(not_refs.iter().map(|s| (*s).to_string()));
    let out = git(path).args(&args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

fn rev_exists(path: &Path, rev: &str) -> bool {
    git(path)
        .args(["rev-parse", "--verify", "--quiet", rev])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn current_branch(path: &Path) -> Option<String> {
    let out = git(path)
        .args(["symbolic-ref", "--short", "-q", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub fn head_mtime(path: &Path) -> Option<SystemTime> {
    let git = path.join(".git");
    let head = if git.is_dir() {
        git.join("HEAD")
    } else if git.is_file() {
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

pub fn is_stale(mtime: Option<SystemTime>, days: u64) -> bool {
    let Some(t) = mtime else {
        return false;
    };
    SystemTime::now()
        .duration_since(t)
        .ok()
        .map(|d| d >= Duration::from_secs(days.saturating_mul(86400)))
        .unwrap_or(false)
}

fn same_path(a: &Path, b: &Path) -> bool {
    let ca = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let cb = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    ca == cb
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_porcelain_flags() {
        let text = "\
worktree /tmp/main
HEAD abc
branch refs/heads/main

worktree /tmp/feat
HEAD def
branch refs/heads/feat/x
locked coding

worktree /tmp/old
HEAD 111
detached
prunable gitdir file points to non-existent location
";
        let list = parse_worktree_porcelain(text);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].branch.as_deref(), Some("main"));
        assert!(!list[0].locked);
        assert_eq!(list[1].branch.as_deref(), Some("feat/x"));
        assert!(list[1].locked);
        assert!(list[2].detached);
        assert!(list[2].prunable);
        assert!(list[2].branch.is_none());
    }

    fn facts(
        primary: bool,
        linked: bool,
        locked: bool,
        dirty: Option<bool>,
        unique: Option<u64>,
        stale: bool,
    ) -> CheckoutFacts {
        CheckoutFacts {
            is_primary: primary,
            is_linked: linked,
            locked,
            dirty,
            detached: false,
            branch: Some("feat".into()),
            unique_vs_main: unique,
            stale,
            mtime: None,
        }
    }

    #[test]
    fn verdict_branches() {
        assert_eq!(
            classify(&facts(true, false, false, Some(false), Some(0), true)),
            WorktreeVerdict::KeepPrimary
        );
        assert_eq!(
            classify(&facts(false, true, false, None, Some(0), true)),
            WorktreeVerdict::KeepUnknown {
                why: "git status failed"
            }
        );
        assert_eq!(
            classify(&facts(false, true, true, Some(false), Some(0), true)),
            WorktreeVerdict::KeepLocked
        );
        assert_eq!(
            classify(&facts(false, true, false, Some(true), Some(0), true)),
            WorktreeVerdict::KeepDirty
        );
        assert_eq!(
            classify(&facts(false, true, false, Some(false), Some(3), true)),
            WorktreeVerdict::KeepUniqueCommits { count: 3 }
        );
        assert_eq!(
            classify(&facts(false, true, false, Some(false), None, true)),
            WorktreeVerdict::KeepUnknown {
                why: "no main/master ref to compare"
            }
        );
        assert_eq!(
            classify(&facts(false, true, false, Some(false), Some(0), false)),
            WorktreeVerdict::KeepRecent
        );
        assert_eq!(
            classify(&facts(false, true, false, Some(false), Some(0), true)),
            WorktreeVerdict::SuggestRemove
        );
        assert_eq!(
            classify(&facts(false, false, false, Some(false), Some(0), true)),
            WorktreeVerdict::SuggestTrashCopy
        );
    }

    #[test]
    fn suggest_only_when_all_safe() {
        let v = classify(&facts(false, true, false, Some(false), Some(0), true));
        assert_eq!(
            v.action(),
            (DefaultAction::SuggestTrash, CleanMethod::GitWorktreeRemove)
        );
        let dirty = classify(&facts(false, true, false, Some(true), Some(0), true));
        assert_eq!(dirty.action(), (DefaultAction::WarnOnly, CleanMethod::None));
    }
}
