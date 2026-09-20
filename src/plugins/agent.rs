use std::path::PathBuf;

use super::AgentPlugin;
use crate::safety::home;

struct Cursor;
struct Codex;
struct Claude;
struct OpenCode;
struct Generic;

impl AgentPlugin for Cursor {
    fn id(&self) -> &'static str {
        "cursor"
    }
    fn display_name(&self) -> &'static str {
        "Cursor"
    }
    fn roots(&self) -> Vec<PathBuf> {
        vec![home().join(".cursor/worktrees")]
    }
}

impl AgentPlugin for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn display_name(&self) -> &'static str {
        "Codex"
    }
    fn roots(&self) -> Vec<PathBuf> {
        vec![home().join(".codex/worktrees")]
    }
}

impl AgentPlugin for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn display_name(&self) -> &'static str {
        "Claude Code"
    }
    fn roots(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

impl AgentPlugin for OpenCode {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn display_name(&self) -> &'static str {
        "OpenCode"
    }
    fn roots(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

impl AgentPlugin for Generic {
    fn id(&self) -> &'static str {
        "generic-agent"
    }
    fn display_name(&self) -> &'static str {
        "unknown / manual worktree"
    }
    fn roots(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

static CURSOR: Cursor = Cursor;
static CODEX: Codex = Codex;
static CLAUDE: Claude = Claude;
static OPENCODE: OpenCode = OpenCode;
static GENERIC: Generic = Generic;

pub fn all() -> Vec<&'static dyn AgentPlugin> {
    vec![&CURSOR, &CODEX, &CLAUDE, &OPENCODE, &GENERIC]
}

pub fn claim_by_path(path: &std::path::Path) -> &'static dyn AgentPlugin {
    let p = path.to_string_lossy();
    if p.contains("/.cursor/worktrees/") {
        return &CURSOR;
    }
    if p.contains("/.codex/worktrees/") {
        return &CODEX;
    }
    if p.contains("/.claude/") {
        return &CLAUDE;
    }
    if p.contains("/.opencode/") {
        return &OPENCODE;
    }
    &GENERIC
}

/// Find git checkouts two levels under an agent root. Grouping dirs are not nodes.
pub fn discover_checkouts(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(l1) = std::fs::read_dir(root) else {
        return out;
    };
    for a in l1.flatten() {
        let p1 = a.path();
        if !p1.is_dir() {
            continue;
        }
        if crate::gitx::is_checkout(&p1) {
            out.push(p1);
            continue;
        }
        let Ok(l2) = std::fs::read_dir(&p1) else {
            continue;
        };
        for b in l2.flatten() {
            let p2 = b.path();
            if p2.is_dir() && crate::gitx::is_checkout(&p2) {
                out.push(p2);
            }
        }
    }
    out
}
