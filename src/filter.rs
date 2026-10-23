use crate::inventory::Kind;
use crate::plugins::{agents, caches, languages};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterKind {
    Lang,
    Agent,
    Cache,
}

impl FilterKind {
    fn flag(self) -> &'static str {
        match self {
            Self::Lang => "--lang",
            Self::Agent => "--agent",
            Self::Cache => "--cache",
        }
    }

    fn matches_kind(self, kind: Kind) -> bool {
        match (self, kind) {
            (Self::Lang, Kind::LanguageArtifact) => true,
            (Self::Agent, Kind::AgentWorkspace) => true,
            (Self::Cache, Kind::Cache) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FilterSpec {
    kind: FilterKind,
    plugin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanFilter {
    specs: Vec<FilterSpec>,
}

impl ScanFilter {
    pub fn open() -> Self {
        Self { specs: Vec::new() }
    }

    pub fn is_open(&self) -> bool {
        self.specs.is_empty()
    }

    pub fn from_flags(
        agent: &[String],
        lang: &[String],
        cache: &[String],
    ) -> Result<Self, String> {
        if agent.is_empty() && lang.is_empty() && cache.is_empty() {
            return Ok(Self::open());
        }
        let mut specs = Vec::new();
        add_flag(&mut specs, FilterKind::Agent, agent)?;
        add_flag(&mut specs, FilterKind::Lang, lang)?;
        add_flag(&mut specs, FilterKind::Cache, cache)?;
        Ok(Self { specs })
    }

    pub fn wants_kind(&self, kind: Kind) -> bool {
        self.is_open() || self.specs.iter().any(|s| s.kind.matches_kind(kind))
    }

    pub fn allows(&self, kind: Kind, plugin: &str) -> bool {
        if self.is_open() {
            return true;
        }
        self.specs.iter().any(|s| {
            s.kind.matches_kind(kind)
                && s.plugin
                    .as_deref()
                    .map(|p| p == plugin)
                    .unwrap_or(true)
        })
    }
}

fn add_flag(
    specs: &mut Vec<FilterSpec>,
    kind: FilterKind,
    raw: &[String],
) -> Result<(), String> {
    for item in raw {
        for part in item.split(',') {
            let part = part.trim().to_ascii_lowercase();
            if part.is_empty() {
                specs.push(FilterSpec {
                    kind,
                    plugin: None,
                });
                continue;
            }
            if !plugin_exists(kind, &part) {
                return Err(unknown_plugin(kind, &part));
            }
            specs.push(FilterSpec {
                kind,
                plugin: Some(part),
            });
        }
    }
    Ok(())
}

fn plugin_exists(kind: FilterKind, plugin: &str) -> bool {
    known_plugins(kind).iter().any(|id| *id == plugin)
}

fn known_plugins(kind: FilterKind) -> Vec<&'static str> {
    match kind {
        FilterKind::Lang => languages().into_iter().map(|p| p.id()).collect(),
        FilterKind::Agent => agents().into_iter().map(|p| p.id()).collect(),
        FilterKind::Cache => caches().iter().map(|p| p.id).collect(),
    }
}

fn unknown_plugin(kind: FilterKind, raw: &str) -> String {
    format!(
        "unknown {} '{}'. try: {}",
        kind.flag(),
        raw,
        known_plugins(kind).join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(agent: &[&str], lang: &[&str], cache: &[&str]) -> ScanFilter {
        ScanFilter::from_flags(
            &agent.iter().map(|s| (*s).to_string()).collect::<Vec<_>>(),
            &lang.iter().map(|s| (*s).to_string()).collect::<Vec<_>>(),
            &cache.iter().map(|s| (*s).to_string()).collect::<Vec<_>>(),
        )
        .unwrap()
    }

    #[test]
    fn bare_flag_is_whole_kind() {
        let agent = flags(&[""], &[], &[]);
        assert!(agent.wants_kind(Kind::AgentWorkspace));
        assert!(!agent.wants_kind(Kind::LanguageArtifact));
        assert!(agent.allows(Kind::AgentWorkspace, "codex"));
        assert!(!agent.allows(Kind::LanguageArtifact, "rust"));
    }

    #[test]
    fn named_plugin() {
        let codex = flags(&["codex"], &[], &[]);
        assert!(codex.allows(Kind::AgentWorkspace, "codex"));
        assert!(!codex.allows(Kind::AgentWorkspace, "cursor"));

        let rust = flags(&[], &["rust"], &[]);
        assert!(rust.allows(Kind::LanguageArtifact, "rust"));
        assert!(!rust.allows(Kind::LanguageArtifact, "node"));
        assert!(!rust.allows(Kind::AgentWorkspace, "codex"));
    }

    #[test]
    fn combine_kinds() {
        let both = flags(&["codex"], &["rust"], &[]);
        assert!(both.allows(Kind::AgentWorkspace, "codex"));
        assert!(both.allows(Kind::LanguageArtifact, "rust"));
        assert!(!both.allows(Kind::AgentWorkspace, "cursor"));
        assert!(!both.allows(Kind::Cache, "npm"));
    }

    #[test]
    fn comma_on_one_flag() {
        let two = flags(&["codex,cursor"], &[], &[]);
        assert!(two.allows(Kind::AgentWorkspace, "codex"));
        assert!(two.allows(Kind::AgentWorkspace, "cursor"));
        assert!(!two.allows(Kind::AgentWorkspace, "claude"));
    }

    #[test]
    fn unknown_is_error() {
        assert!(ScanFilter::from_flags(&["nope".into()], &[], &[]).is_err());
        assert!(ScanFilter::from_flags(&[], &["codex".into()], &[]).is_err());
    }

    #[test]
    fn open_allows_all() {
        let open = ScanFilter::open();
        assert!(open.allows(Kind::Cache, "docker"));
        assert!(open.wants_kind(Kind::Unknown));
        assert_eq!(ScanFilter::from_flags(&[], &[], &[]).unwrap(), open);
    }
}
