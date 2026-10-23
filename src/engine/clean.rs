use std::path::{Path, PathBuf};

use crate::gitx;
use crate::inventory::{CleanMethod, DefaultAction, Kind, Node};
use crate::plugins::agent;
use crate::safety::{can_emit_node, can_trash};

/// Print intended actions. Never deletes, never calls git worktree remove, never uses Trash.
pub fn dry_run(nodes: &[&Node]) -> String {
    let mut lines = vec![
        "[dry-run] gomi will not delete files or move anything to Trash.".to_string(),
        "no --apply; official cleanup commands are not executed.".to_string(),
        String::new(),
    ];
    if nodes.is_empty() {
        lines.push("no matching entries.".into());
        return lines.join("\n");
    }
    for n in nodes {
        let verdict = match n.default_action {
            DefaultAction::SuggestTrash => match n.clean_method {
                CleanMethod::Trash => "if apply is authorized: move to Trash",
                CleanMethod::GitWorktreeRemove => "if apply is authorized: git worktree remove",
                CleanMethod::OfficialCmd => "if apply is authorized: print official command only",
                CleanMethod::None => "will not delete even if authorized",
            },
            DefaultAction::InspectOnly => "scan does not suggest delete; inspect first",
            DefaultAction::WarnOnly => "git safety did not pass; warn only",
            DefaultAction::None => "blocked by safety policy",
        };
        lines.push(format!(
            "#{}  {:<18}  {:>10}  {}  [{} / {}]",
            n.id,
            n.plugin_id,
            crate::inventory::human(n.size_bytes),
            n.path.display(),
            n.default_action.as_str(),
            verdict
        ));
        if !n.explain.what.is_empty() {
            lines.push(format!("         {}", n.explain.what));
        }
    }
    lines.join("\n")
}

pub fn selectable(n: &Node) -> bool {
    n.default_action == DefaultAction::SuggestTrash
        && matches!(
            n.clean_method,
            CleanMethod::Trash | CleanMethod::GitWorktreeRemove
        )
        && can_emit_node(&n.path)
}

#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub id: String,
    pub path: PathBuf,
    pub method: CleanMethod,
}

#[derive(Debug, Clone)]
pub struct ApplySkip {
    pub id: String,
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyReport {
    pub done: Vec<ApplyOutcome>,
    pub skipped: Vec<ApplySkip>,
}

impl ApplyReport {
    pub fn summary(&self) -> String {
        let moved = self.done.len();
        let skipped = self.skipped.len();
        let mut s = format!("done {moved}, skipped {skipped}");
        if let Some(first) = self.skipped.first() {
            s.push_str(&format!("; first skip: {}", first.reason));
        }
        s
    }
}

/// Move eligible items to Trash, or `git worktree remove` for linked worktrees.
/// Never calls `rm` / `remove_dir_all`.
pub fn apply(nodes: &[&Node], stale_after_days: u64) -> ApplyReport {
    let mut report = ApplyReport::default();
    let selected = drop_covered_descendants(nodes);
    for n in selected {
        match apply_one(n, stale_after_days) {
            Ok(done) => report.done.push(done),
            Err(skip) => report.skipped.push(skip),
        }
    }
    report
}

fn drop_covered_descendants<'a>(nodes: &[&'a Node]) -> Vec<&'a Node> {
    let mut ordered = nodes.to_vec();
    ordered.sort_by_key(|n| n.path.as_os_str().len());
    let mut keep: Vec<&Node> = Vec::new();
    for n in ordered {
        if keep
            .iter()
            .any(|parent| n.path != parent.path && n.path.starts_with(&parent.path))
        {
            continue;
        }
        keep.push(n);
    }
    keep
}

fn apply_one(n: &Node, stale_after_days: u64) -> Result<ApplyOutcome, ApplySkip> {
    let skip = |reason: String| ApplySkip {
        id: n.id.clone(),
        path: n.path.clone(),
        reason,
    };
    if n.default_action != DefaultAction::SuggestTrash {
        return Err(skip("not suggest_trash".into()));
    }
    if !matches!(
        n.clean_method,
        CleanMethod::Trash | CleanMethod::GitWorktreeRemove
    ) {
        return Err(skip(format!(
            "method {} is not trash or worktree remove",
            n.clean_method.as_str()
        )));
    }
    if !n.path.exists() {
        return Err(skip("path is gone".into()));
    }
    if !can_trash(&n.path) {
        return Err(skip("blocked by safety kernel".into()));
    }
    let method = match recheck(n, stale_after_days) {
        Ok(method) => method,
        Err(reason) => return Err(skip(reason)),
    };
    match method {
        CleanMethod::Trash => move_to_trash(&n.path).map_err(skip)?,
        CleanMethod::GitWorktreeRemove => gitx::worktree_remove(&n.path).map_err(skip)?,
        CleanMethod::OfficialCmd | CleanMethod::None => {
            return Err(skip("method is not executable here".into()));
        }
    }
    Ok(ApplyOutcome {
        id: n.id.clone(),
        path: n.path.clone(),
        method,
    })
}

fn recheck(n: &Node, stale_after_days: u64) -> Result<CleanMethod, String> {
    if n.kind == Kind::AgentWorkspace {
        let facts = gitx::inspect_checkout(
            &n.path,
            stale_after_days,
            agent::under_agent_root(&n.path),
        );
        let verdict = gitx::classify(&facts);
        let (action, method) = verdict.action();
        if action != DefaultAction::SuggestTrash {
            return Err(verdict.reason());
        }
        if !matches!(
            method,
            CleanMethod::Trash | CleanMethod::GitWorktreeRemove
        ) {
            return Err("worktree is no longer trashable".into());
        }
        return Ok(method);
    }
    if !can_emit_node(&n.path) {
        return Err("blocked by safety kernel".into());
    }
    Ok(n.clean_method)
}

fn move_to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{Explain, Kind};
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn explain() -> Explain {
        Explain {
            what: "test".into(),
            if_deleted: "gone".into(),
            how_rebuild: "rebuild".into(),
        }
    }

    fn node(action: DefaultAction, method: CleanMethod, path: PathBuf, kind: Kind) -> Node {
        Node {
            id: path.display().to_string(),
            path,
            kind,
            plugin_id: "test".into(),
            parent_id: None,
            size_bytes: 1,
            mtime: None,
            default_action: action,
            clean_method: method,
            rebuildable: true,
            git_dirty: None,
            stale: None,
            explain: explain(),
            child_summary: String::new(),
            children: Vec::new(),
        }
    }

    fn unique_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("gomi-apply-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("keep-me.txt"), "x").unwrap();
        dir
    }

    #[test]
    fn selectable_only_suggest_trash() {
        let dir = unique_dir();
        let ok = node(
            DefaultAction::SuggestTrash,
            CleanMethod::Trash,
            dir.clone(),
            Kind::LanguageArtifact,
        );
        assert!(selectable(&ok));
        let warn = node(
            DefaultAction::WarnOnly,
            CleanMethod::Trash,
            dir.clone(),
            Kind::LanguageArtifact,
        );
        assert!(!selectable(&warn));
        let official = node(
            DefaultAction::SuggestTrash,
            CleanMethod::OfficialCmd,
            dir,
            Kind::Cache,
        );
        assert!(!selectable(&official));
    }

    #[test]
    fn apply_refuses_warn_only_and_does_not_touch() {
        let dir = unique_dir();
        let n = node(
            DefaultAction::WarnOnly,
            CleanMethod::Trash,
            dir.clone(),
            Kind::LanguageArtifact,
        );
        let report = apply(&[&n], 7);
        assert!(report.done.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert!(dir.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_refuses_home() {
        let home = crate::safety::home();
        let n = node(
            DefaultAction::SuggestTrash,
            CleanMethod::Trash,
            home,
            Kind::LanguageArtifact,
        );
        let report = apply(&[&n], 7);
        assert!(report.done.is_empty());
        assert!(report.skipped.iter().any(|s| s.reason.contains("safety")));
    }

    #[test]
    fn drop_parent_covers_child() {
        let parent = node(
            DefaultAction::SuggestTrash,
            CleanMethod::Trash,
            PathBuf::from("/tmp/gomi-parent"),
            Kind::AgentWorkspace,
        );
        let child = node(
            DefaultAction::SuggestTrash,
            CleanMethod::Trash,
            PathBuf::from("/tmp/gomi-parent/target"),
            Kind::LanguageArtifact,
        );
        let kept = drop_covered_descendants(&[&parent, &child]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path, parent.path);
    }

    #[test]
    fn apply_moves_dir_to_trash() {
        let dir = unique_dir();
        let n = node(
            DefaultAction::SuggestTrash,
            CleanMethod::Trash,
            dir.clone(),
            Kind::LanguageArtifact,
        );
        let report = apply(&[&n], 7);
        assert!(
            report.skipped.is_empty(),
            "{}",
            report.summary()
        );
        assert_eq!(report.done.len(), 1);
        assert!(!dir.exists());
        assert_eq!(report.done[0].method, CleanMethod::Trash);
    }

    #[test]
    fn apply_worktree_remove_unregisters() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gomi-wt-{stamp}"));
        let main = root.join("main");
        let wt = root.join("linked");
        fs::create_dir_all(&main).unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .current_dir(&main)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init"]);
        git(&["config", "user.email", "gomi@test"]);
        git(&["config", "user.name", "gomi"]);
        fs::write(main.join("f"), "x").unwrap();
        git(&["add", "f"]);
        git(&["commit", "-m", "init"]);
        git(&["branch", "-M", "main"]);
        let out = Command::new("git")
            .current_dir(&main)
            .args(["worktree", "add", wt.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        // Make it stale enough for recheck.
        let old = UNIX_EPOCH + std::time::Duration::from_secs(1);
        let _ = filetime_touch(&wt, old);
        let n = node(
            DefaultAction::SuggestTrash,
            CleanMethod::GitWorktreeRemove,
            wt.clone(),
            Kind::AgentWorkspace,
        );
        let report = apply(&[&n], 0);
        if !report.skipped.is_empty() {
            let _ = fs::remove_dir_all(&root);
            panic!("{}", report.summary());
        }
        assert!(!wt.exists());
        let list = gitx::worktree_list(&main);
        assert_eq!(list.iter().filter(|w| !w.is_primary).count(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    fn filetime_touch(path: &Path, t: SystemTime) -> std::io::Result<()> {
        let file = fs::File::open(path)?;
        file.set_modified(t)
    }
}
