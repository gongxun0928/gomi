use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::config::Config;
use crate::gitx;
use crate::inventory::{
    path_id, CleanMethod, DefaultAction, Explain, Kind, Node,
};
use crate::plugins::agent::{claim_by_path, discover_checkouts};
use crate::plugins::cache::expand_exact;
use crate::plugins::language::claim as claim_language;
use crate::plugins::{agent, caches, CacheClean};
use crate::progress::{default_progress, ScanProgress};
use crate::safety::{can_emit_node, can_walk, is_forbidden_scan_root};

const SKIP_DESCEND: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    ".venv",
    "__pycache__",
    ".build",
    "vendor",
];

pub struct Inventory {
    pub roots: Vec<Node>,
}

impl Inventory {
    pub fn find(&self, id: &str) -> Option<&Node> {
        self.roots.iter().find_map(|n| n.find(id))
    }

    pub fn find_by_path(&self, path: &Path) -> Option<&Node> {
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.roots.iter().find_map(|n| find_path(n, &canon))
    }
}

fn find_path<'a>(n: &'a Node, path: &Path) -> Option<&'a Node> {
    if n.path == path {
        return Some(n);
    }
    n.children.iter().find_map(|c| find_path(c, path))
}

pub fn scan(cfg: &Config) -> Inventory {
    let mut progress = default_progress();
    let inv = scan_with(cfg, progress.as_mut());
    progress.finish();
    inv
}

pub fn scan_with(cfg: &Config, progress: &mut dyn ScanProgress) -> Inventory {
    let mut covered: HashSet<PathBuf> = HashSet::new();
    let mut roots = Vec::new();

    if cfg.include_hotspots {
        for checkout in discover_agent_checkouts(cfg) {
            if !covered.insert(checkout.clone()) {
                continue;
            }
            if let Some(node) = agent_node(&checkout, cfg, progress) {
                roots.push(node);
            }
        }
    }

    let agent_paths: Vec<PathBuf> = roots.iter().map(|n| n.path.clone()).collect();

    for proj in &cfg.project_roots {
        if is_forbidden_scan_root(proj) {
            continue;
        }
        progress.walking(proj, None);
        if cfg.include_hotspots {
            collect_git_worktrees(proj, &mut covered, &mut roots, cfg, progress);
        }
        let arts = language_under(proj, Some(proj), &agent_paths, progress, true);
        roots.extend(arts);
    }

    if cfg.include_hotspots {
        roots.extend(cache_nodes(progress));
    }
    roots.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    Inventory { roots }
}

fn measure(
    path: &Path,
    label: &str,
    add_to_total: bool,
    progress: &mut dyn ScanProgress,
) -> u64 {
    progress.sizing(path, label, 0, add_to_total);
    crate::inventory::du_with(path, &mut |_p, running| {
        progress.sizing(path, label, running, add_to_total);
    })
}

fn label(kind: &str, plugin: &str) -> String {
    format!("{kind}/{plugin}")
}

fn discover_agent_checkouts(cfg: &Config) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for plugin in agent::all() {
        for root in plugin.roots() {
            if root.is_dir() {
                out.extend(discover_checkouts(&root));
            }
        }
    }
    for extra in &cfg.extra_watch {
        if extra.is_dir() {
            out.extend(discover_checkouts(extra));
            if gitx::is_checkout(extra) {
                out.push(extra.clone());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn collect_git_worktrees(
    proj: &Path,
    covered: &mut HashSet<PathBuf>,
    roots: &mut Vec<Node>,
    cfg: &Config,
    progress: &mut dyn ScanProgress,
) {
    walk_git_repos(proj, &mut HashSet::new(), 0, &mut |repo| {
        for wt in gitx::worktree_list(repo) {
            if wt.is_primary {
                continue;
            }
            if !covered.insert(wt.path.clone()) {
                continue;
            }
            if let Some(node) = agent_node(&wt.path, cfg, progress) {
                roots.push(node);
            }
        }
    });
}

fn walk_git_repos(dir: &Path, seen: &mut HashSet<PathBuf>, depth: u32, on_repo: &mut impl FnMut(&Path)) {
    if depth > 12 || !can_walk(dir) {
        return;
    }
    let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if SKIP_DESCEND.contains(&name) && depth > 0 {
        return;
    }
    if gitx::is_primary_checkout(dir) {
        on_repo(dir);
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() && seen.insert(p.clone()) {
            walk_git_repos(&p, seen, depth + 1, on_repo);
        }
    }
}

fn agent_node(checkout: &Path, cfg: &Config, progress: &mut dyn ScanProgress) -> Option<Node> {
    if !can_emit_node(checkout) || !checkout.is_dir() {
        return None;
    }
    let plugin = claim_by_path(checkout);
    let primary = gitx::is_primary_checkout(checkout) && !under_agent_root(checkout);
    let dirty = gitx::is_dirty(checkout);
    let mtime = gitx::dir_mtime(checkout);
    let stale = is_stale(mtime, cfg.stale_after_days);
    progress.walking(checkout, None);
    let children = language_under(checkout, Some(checkout), &[], progress, false);
    let size = measure(checkout, &label("agent", plugin.id()), true, progress);
    let (action, method) = if primary {
        (DefaultAction::None, CleanMethod::None)
    } else if dirty {
        (DefaultAction::WarnOnly, CleanMethod::GitWorktreeRemove)
    } else if !stale {
        (DefaultAction::WarnOnly, CleanMethod::GitWorktreeRemove)
    } else if gitx::is_linked_worktree(checkout) {
        (DefaultAction::SuggestTrash, CleanMethod::GitWorktreeRemove)
    } else {
        (DefaultAction::SuggestTrash, CleanMethod::Trash)
    };
    let n_art = children.len();
    let child_summary = if n_art == 0 {
        String::new()
    } else {
        format!("{n_art} rebuildable artifact(s)")
    };
    let id = path_id(checkout);
    let mut node = Node {
        id: id.clone(),
        path: checkout.to_path_buf(),
        kind: Kind::AgentWorkspace,
        plugin_id: plugin.id().to_string(),
        parent_id: None,
        size_bytes: size,
        mtime,
        default_action: action,
        clean_method: method,
        rebuildable: false,
        git_dirty: Some(dirty),
        stale: Some(stale),
        explain: Explain {
            what: format!(
                "{} workspace ({}); primary checkout is not deletable.",
                plugin.display_name(),
                if primary { "primary" } else { "copy / worktree" }
            ),
            if_deleted: "Would Trash or git worktree remove the whole tree; may contain local files. This build is dry-run only."
                .into(),
            how_rebuild: "Recreate the worktree from the main repo / rerun the agent.".into(),
        },
        child_summary,
        children,
    };
    for c in &mut node.children {
        c.parent_id = Some(id.clone());
    }
    progress.found(
        checkout,
        &label("agent", plugin.id()),
        size,
        true,
    );
    Some(node)
}

fn under_agent_root(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/.cursor/worktrees/") || s.contains("/.codex/worktrees/")
}

fn language_under(
    dir: &Path,
    stop: Option<&Path>,
    skip_under: &[PathBuf],
    progress: &mut dyn ScanProgress,
    add_to_total: bool,
) -> Vec<Node> {
    let stop = stop.unwrap_or(dir);
    let mut out = Vec::new();
    walk_language(dir, stop, skip_under, 0, &mut out, progress, add_to_total);
    out.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    out
}

fn walk_language(
    dir: &Path,
    stop: &Path,
    skip_under: &[PathBuf],
    depth: u32,
    out: &mut Vec<Node>,
    progress: &mut dyn ScanProgress,
    add_to_total: bool,
) {
    if depth > 18 || !can_walk(dir) {
        return;
    }
    if depth == 0 {
        progress.walking(dir, None);
    }
    if skip_under.iter().any(|s| dir == s || dir.starts_with(s)) && depth > 0 {
        return;
    }
    let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if depth > 0 {
        if let Some((lang, art, _)) = claim_language(dir, stop) {
            if can_emit_node(dir) {
                out.push(language_node(dir, lang, art, progress, add_to_total));
            }
            return;
        }
        if SKIP_DESCEND.contains(&name) {
            return;
        }
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            walk_language(
                &p,
                stop,
                skip_under,
                depth + 1,
                out,
                progress,
                add_to_total,
            );
        }
    }
}

fn language_node(
    path: &Path,
    lang: &dyn crate::plugins::LanguagePlugin,
    art: &crate::plugins::ArtifactSpec,
    progress: &mut dyn ScanProgress,
    add_to_total: bool,
) -> Node {
    let (what, if_deleted, how_rebuild) = lang.explain(art.id);
    let node = Node {
        id: path_id(path),
        path: path.to_path_buf(),
        kind: Kind::LanguageArtifact,
        plugin_id: lang.id().to_string(),
        parent_id: None,
        size_bytes: measure(path, &label("lang", lang.id()), add_to_total, progress),
        mtime: std::fs::metadata(path).and_then(|m| m.modified()).ok(),
        default_action: DefaultAction::SuggestTrash,
        clean_method: CleanMethod::Trash,
        rebuildable: true,
        git_dirty: None,
        stale: None,
        explain: Explain {
            what,
            if_deleted: format!("{if_deleted} This build is dry-run only."),
            how_rebuild,
        },
        child_summary: String::new(),
        children: Vec::new(),
    };
    progress.found(
        path,
        &label("lang", lang.id()),
        node.size_bytes,
        add_to_total,
    );
    node
}

fn cache_nodes(progress: &mut dyn ScanProgress) -> Vec<Node> {
    let mut out = Vec::new();
    let home = crate::safety::home();
    for spec in caches() {
        let mut paths = Vec::new();
        for pat in spec.exact_paths {
            let p = expand_exact(pat);
            if p.exists() {
                paths.push(p);
            }
        }
        for name in spec.named_library_caches {
            let p = home.join("Library/Caches").join(name);
            if p.exists() {
                paths.push(p);
            }
        }
        for name in spec.named_dot_caches {
            let p = home.join(".cache").join(name);
            if p.exists() {
                paths.push(p);
            }
        }
        paths.sort();
        paths.dedup();
        for path in paths {
            if !can_emit_node(&path) {
                continue;
            }
            let method = match spec.clean {
                CacheClean::Trash => CleanMethod::Trash,
                CacheClean::OfficialCmd => CleanMethod::OfficialCmd,
                CacheClean::None => CleanMethod::None,
            };
            progress.walking(&path, None);
            let size = measure(&path, &label("cache", spec.id), true, progress);
            progress.found(&path, &label("cache", spec.id), size, true);
            out.push(Node {
                id: path_id(&path),
                path: path.clone(),
                kind: Kind::Cache,
                plugin_id: spec.id.to_string(),
                parent_id: None,
                size_bytes: size,
                mtime: std::fs::metadata(&path).and_then(|m| m.modified()).ok(),
                default_action: DefaultAction::InspectOnly,
                clean_method: method,
                rebuildable: true,
                git_dirty: None,
                stale: None,
                explain: Explain {
                    what: spec.what.into(),
                    if_deleted: format!(
                        "{} This build is dry-run only.{}",
                        spec.if_deleted,
                        spec.official_cmd
                            .map(|c| format!(" official cmd: {c}"))
                            .unwrap_or_default()
                    ),
                    how_rebuild: spec.how_rebuild.into(),
                },
                child_summary: spec.display_name.into(),
                children: Vec::new(),
            });
        }
    }
    out
}

fn is_stale(mtime: Option<SystemTime>, days: u64) -> bool {
    let Some(t) = mtime else {
        return false;
    };
    SystemTime::now()
        .duration_since(t)
        .ok()
        .map(|d| d >= Duration::from_secs(days.saturating_mul(86400)))
        .unwrap_or(false)
}
