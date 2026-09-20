use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    LanguageArtifact,
    AgentWorkspace,
    Cache,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultAction {
    SuggestTrash,
    InspectOnly,
    WarnOnly,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanMethod {
    Trash,
    OfficialCmd,
    GitWorktreeRemove,
    None,
}

impl CleanMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            CleanMethod::Trash => "trash",
            CleanMethod::OfficialCmd => "official_cmd",
            CleanMethod::GitWorktreeRemove => "git_worktree_remove",
            CleanMethod::None => "none",
        }
    }
}

impl DefaultAction {
    pub fn as_str(self) -> &'static str {
        match self {
            DefaultAction::SuggestTrash => "suggest_trash",
            DefaultAction::InspectOnly => "inspect_only",
            DefaultAction::WarnOnly => "warn_only",
            DefaultAction::None => "none",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Explain {
    pub what: String,
    pub if_deleted: String,
    pub how_rebuild: String,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub path: PathBuf,
    pub kind: Kind,
    pub plugin_id: String,
    pub parent_id: Option<String>,
    pub size_bytes: u64,
    pub mtime: Option<SystemTime>,
    pub default_action: DefaultAction,
    pub clean_method: CleanMethod,
    pub rebuildable: bool,
    pub git_dirty: Option<bool>,
    pub stale: Option<bool>,
    pub explain: Explain,
    pub child_summary: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn walk_flat(&self) -> Vec<&Node> {
        let mut out = vec![self];
        for c in &self.children {
            out.extend(c.walk_flat());
        }
        out
    }

    pub fn find(&self, id: &str) -> Option<&Node> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }
}
