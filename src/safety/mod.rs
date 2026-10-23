mod catalog;
mod kernel;

pub use catalog::{classify, home, is_forbidden_scan_root, SafetyClass};
pub use kernel::{can_emit_node, can_trash, can_walk};
