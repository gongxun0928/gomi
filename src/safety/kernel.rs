use std::path::Path;

use super::catalog::{classify, is_forbidden_scan_root, SafetyClass};

/// Dangerous paths themselves never appear in results.
pub fn can_emit_node(path: &Path) -> bool {
    match classify(path) {
        SafetyClass::Allow => true,
        SafetyClass::Tier1 | SafetyClass::Tier2 => false,
    }
}

pub fn can_walk(path: &Path) -> bool {
    match classify(path) {
        SafetyClass::Tier1 => false,
        SafetyClass::Tier2 => true,
        SafetyClass::Allow => true,
    }
}

/// Extra checks before moving anything to Trash or removing a worktree.
pub fn can_trash(path: &Path) -> bool {
    if !path.exists() || !can_emit_node(path) || is_forbidden_scan_root(path) {
        return false;
    }
    let canon = super::catalog::canonicalize_existing_or_self(path);
    canon.parent().is_some() && canon.components().count() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::home;

    #[test]
    fn trash_refuses_home_and_root() {
        assert!(!can_trash(&home()));
        assert!(!can_trash(Path::new("/")));
    }
}
