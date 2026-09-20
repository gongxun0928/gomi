pub mod agent;
pub mod cache;
pub mod language;

#[derive(Debug, Clone, Copy)]
pub struct ArtifactSpec {
    pub id: &'static str,
    pub exact_names: &'static [&'static str],
    pub prefixes: &'static [&'static str],
}

impl ArtifactSpec {
    pub fn matches(&self, dir_name: &str) -> bool {
        self.exact_names.contains(&dir_name) || self.prefixes.iter().any(|p| dir_name.starts_with(p))
    }
}

pub trait LanguagePlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn project_markers(&self) -> &'static [&'static str];
    fn artifacts(&self) -> &'static [ArtifactSpec];
    fn explain(&self, artifact_id: &str) -> (String, String, String);
}

pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn roots(&self) -> Vec<std::path::PathBuf>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheClean {
    Trash,
    OfficialCmd,
    None,
}

pub struct CacheSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub exact_paths: &'static [&'static str],
    pub named_library_caches: &'static [&'static str],
    pub named_dot_caches: &'static [&'static str],
    pub clean: CacheClean,
    pub official_cmd: Option<&'static str>,
    pub what: &'static str,
    pub if_deleted: &'static str,
    pub how_rebuild: &'static str,
}

pub fn languages() -> Vec<&'static dyn LanguagePlugin> {
    language::all()
}

pub fn agents() -> Vec<&'static dyn AgentPlugin> {
    agent::all()
}

pub fn caches() -> &'static [CacheSpec] {
    cache::all()
}
