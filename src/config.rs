use std::path::PathBuf;

use crate::filter::ScanFilter;
use crate::safety::home;

#[derive(Debug, Clone)]
pub struct Config {
    pub project_roots: Vec<PathBuf>,
    pub extra_watch: Vec<PathBuf>,
    pub stale_after_days: u64,
    pub include_hotspots: bool,
    pub filter: ScanFilter,
}

impl Config {
    pub fn load(extra_roots: &[PathBuf], include_hotspots: bool) -> Self {
        let mut roots = Vec::new();
        if extra_roots.is_empty() {
            if let Ok(cwd) = std::env::current_dir() {
                if !crate::safety::is_forbidden_scan_root(&cwd) {
                    roots.push(cwd);
                }
            }
            let workspace = home().join("workspace");
            if workspace.is_dir() {
                roots.push(workspace);
            }
        } else {
            roots.extend(extra_roots.iter().cloned());
        }
        roots = dedup_roots(roots);
        Self {
            project_roots: roots,
            extra_watch: Vec::new(),
            stale_after_days: 7,
            include_hotspots,
            filter: ScanFilter::open(),
        }
    }

    pub fn with_filter(mut self, filter: ScanFilter) -> Self {
        self.filter = filter;
        self
    }
}

fn dedup_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut canon: Vec<PathBuf> = roots
        .into_iter()
        .filter_map(|r| std::fs::canonicalize(&r).ok().or(Some(r)))
        .collect();
    canon.sort();
    canon.dedup();
    canon
        .clone()
        .into_iter()
        .filter(|p| {
            !canon
                .iter()
                .any(|o| o != p && p.starts_with(o))
        })
        .collect()
}
