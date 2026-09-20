use std::path::Path;

use super::catalog::{classify, SafetyClass};

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
