use std::path::{Path, PathBuf};

use dirs::home_dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyClass {
    /// Hard deny: no node, no walk, no break-glass.
    Tier1,
    /// Parent itself is silent; children may still be claimed.
    Tier2,
    Allow,
}

pub fn home() -> PathBuf {
    home_dir().unwrap_or_else(|| PathBuf::from("/Users/unknown"))
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else if p == "~" {
        home()
    } else {
        PathBuf::from(p)
    }
}

#[allow(dead_code)]
pub fn tier1_prefixes() -> Vec<PathBuf> {
    let h = home();
    let mut v = vec![
        PathBuf::from("/System"),
        PathBuf::from("/bin"),
        PathBuf::from("/sbin"),
        PathBuf::from("/usr"),
        PathBuf::from("/private/etc"),
        PathBuf::from("/private/var/db"),
        PathBuf::from("/Library"),
        PathBuf::from("/"),
        h.clone(),
        h.join("Library/Keychains"),
        h.join(".ssh"),
        h.join(".gnupg"),
        PathBuf::from("/.MobileBackups"),
    ];
    v.sort_by(|a, b| b.as_os_str().len().cmp(&a.as_os_str().len()));
    v
}

pub fn tier2_prefixes() -> Vec<PathBuf> {
    let h = home();
    vec![
        h.join("Documents"),
        h.join("Desktop"),
        h.join("Pictures"),
        h.join("Movies"),
        h.join("Music"),
        h.join("Library"),
        h.join(".kube"),
        h.join(".aws"),
        h.join(".cursor"),
        h.join(".claude"),
        h.join(".codex"),
        h.join(".opencode"),
    ]
}

/// Prefixes plugins may claim (more specific than Tier2).
pub fn plugin_child_allowlist() -> Vec<PathBuf> {
    let h = home();
    vec![
        h.join(".cursor/worktrees"),
        h.join(".codex/worktrees"),
        h.join("Library/Developer/Xcode/DerivedData"),
        h.join("Library/Developer/CoreSimulator"),
        h.join("Library/Developer/Xcode/Archives"),
        h.join("Library/Caches"),
        h.join(".cargo/registry"),
        h.join(".cargo/git"),
        h.join(".cargo/bin"),
        h.join(".rustup/toolchains"),
        h.join(".npm"),
        h.join(".docker"),
        h.join(".m2/repository"),
        h.join(".gradle/caches"),
        h.join(".swiftpm"),
        h.join(".cache"),
        h.join("go/pkg/mod"),
    ]
}

pub fn classify(path: &Path) -> SafetyClass {
    let canon = canonicalize_existing_or_self(path);
    let h = home();
    if canon == Path::new("/") || canon == h {
        return SafetyClass::Tier1;
    }
    for p in [
        h.join("Library/Keychains"),
        h.join(".ssh"),
        h.join(".gnupg"),
        PathBuf::from("/System"),
        PathBuf::from("/bin"),
        PathBuf::from("/sbin"),
        PathBuf::from("/usr"),
        PathBuf::from("/private/etc"),
        PathBuf::from("/private/var/db"),
        PathBuf::from("/Library"),
        PathBuf::from("/.MobileBackups"),
    ] {
        if path_is_or_under(&canon, &p) {
            return SafetyClass::Tier1;
        }
    }
    for allow in plugin_child_allowlist() {
        if path_is_or_under(&canon, &allow) {
            return SafetyClass::Allow;
        }
    }
    for p in tier2_prefixes() {
        if canon == p {
            return SafetyClass::Tier2;
        }
        if path_is_or_under(&canon, &p) {
            // Tier2 parent; children stay walkable (artifacts on Desktop).
            return SafetyClass::Allow;
        }
    }
    SafetyClass::Allow
}

pub fn is_forbidden_scan_root(path: &Path) -> bool {
    let canon = canonicalize_existing_or_self(path);
    canon == Path::new("/") || canon == home() || matches!(classify(&canon), SafetyClass::Tier1)
}

pub fn path_is_or_under(path: &Path, prefix: &Path) -> bool {
    path == prefix || path.starts_with(prefix)
}

pub fn canonicalize_existing_or_self(path: &Path) -> PathBuf {
    dunce_canon(path).unwrap_or_else(|| path.to_path_buf())
}

fn dunce_canon(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

#[allow(dead_code)]
pub fn expand_user(p: &str) -> PathBuf {
    expand_home(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_tier1() {
        assert_eq!(classify(&home()), SafetyClass::Tier1);
    }

    #[test]
    fn ssh_is_tier1() {
        assert_eq!(classify(&home().join(".ssh")), SafetyClass::Tier1);
    }

    #[test]
    fn cursor_worktree_is_allow() {
        assert_eq!(
            classify(&home().join(".cursor/worktrees/demo/abc")),
            SafetyClass::Allow
        );
    }

    #[test]
    fn cursor_root_is_tier2() {
        assert_eq!(classify(&home().join(".cursor")), SafetyClass::Tier2);
    }
}
