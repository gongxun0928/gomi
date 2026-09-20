use std::collections::BTreeMap;

use crate::inventory::{human, Kind, Node};

pub fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::LanguageArtifact => "lang",
        Kind::AgentWorkspace => "agent",
        Kind::Cache => "cache",
        Kind::Unknown => "unknown",
    }
}

#[derive(Debug, Clone)]
pub struct TreeItem {
    pub id: String,
    pub title: String,
    pub bytes: u64,
    pub count: usize,
    pub children: Vec<TreeItem>,
    pub node: Option<Node>,
}

pub fn build_tree(roots: Vec<Node>) -> Vec<TreeItem> {
    let mut by_kind: BTreeMap<&'static str, BTreeMap<String, Vec<Node>>> = BTreeMap::new();
    for root in roots {
        collect_for_groups(root, &mut by_kind);
    }

    let order = ["lang", "agent", "cache", "unknown"];
    let mut out = Vec::new();
    for kind in order {
        let Some(plugins) = by_kind.remove(kind) else {
            continue;
        };
        let mut plugin_items = Vec::new();
        let mut kind_bytes = 0u64;
        let mut kind_count = 0usize;
        for (plugin, nodes) in plugins {
            let mut children = Vec::new();
            let mut plugin_bytes = 0u64;
            let plugin_count = nodes.len();
            for node in nodes {
                plugin_bytes = plugin_bytes.saturating_add(node.size_bytes);
                children.push(item_from_node(node));
            }
            children.sort_by(|a, b| b.bytes.cmp(&a.bytes));
            kind_bytes = kind_bytes.saturating_add(plugin_bytes);
            kind_count += plugin_count;
            plugin_items.push(TreeItem {
                id: format!("{kind}/{plugin}"),
                title: format!("{kind}/{plugin}"),
                bytes: plugin_bytes,
                count: plugin_count,
                children,
                node: None,
            });
        }
        plugin_items.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        out.push(TreeItem {
            id: kind.to_string(),
            title: kind.to_string(),
            bytes: kind_bytes,
            count: kind_count,
            children: plugin_items,
            node: None,
        });
    }
    out
}

fn collect_for_groups(
    node: Node,
    by_kind: &mut BTreeMap<&'static str, BTreeMap<String, Vec<Node>>>,
) {
    let kind = kind_key(node.kind);
    match node.kind {
        Kind::AgentWorkspace => {
            let children = node.children.clone();
            by_kind
                .entry(kind)
                .or_default()
                .entry(node.plugin_id.clone())
                .or_default()
                .push(node);
            for child in children {
                collect_for_groups(child, by_kind);
            }
        }
        Kind::LanguageArtifact | Kind::Cache | Kind::Unknown => {
            by_kind
                .entry(kind)
                .or_default()
                .entry(node.plugin_id.clone())
                .or_default()
                .push(node);
        }
    }
}

fn item_from_node(node: Node) -> TreeItem {
    let name = node
        .path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| node.path.display().to_string());
    let children = node
        .children
        .iter()
        .cloned()
        .map(item_from_node)
        .collect();
    TreeItem {
        id: format!("node:{}", node.id),
        title: name,
        bytes: node.size_bytes,
        count: 1,
        children,
        node: Some(node),
    }
}

const NAME_WIDTH: usize = 24;
const SIZE_WIDTH: usize = 10;
const COUNT_WIDTH: usize = 5;

pub fn format_cluster(title: &str, bytes: u64, count: usize, indent: usize) -> String {
    let name = format!("{}{title}", " ".repeat(indent));
    format!(
        "{name:<NAME_WIDTH$}  {:>SIZE_WIDTH$}  {:>COUNT_WIDTH$}",
        human(bytes),
        count,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeTier {
    Hot,
    Warm,
    Cool,
}

pub fn size_tier(bytes: u64) -> SizeTier {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB200: u64 = 200 * 1024 * 1024;
    if bytes >= GB {
        SizeTier::Hot
    } else if bytes >= MB200 {
        SizeTier::Warm
    } else {
        SizeTier::Cool
    }
}

pub fn paint_line(line: &str, bytes: u64) -> String {
    let code = match size_tier(bytes) {
        SizeTier::Hot => "\x1b[1;31m",
        SizeTier::Warm => "\x1b[1;33m",
        SizeTier::Cool => "\x1b[1;32m",
    };
    format!("{code}{line}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_columns_line_up() {
        let a = format_cluster("lang", 96_900_000_000, 55, 0);
        let b = format_cluster("lang/cpp", 146_300_000, 28, 2);
        let c = format_cluster("agent/generic-agent", 7_000_000_000, 2, 2);
        for line in [&a, &b, &c] {
            assert_eq!(line.as_bytes()[NAME_WIDTH], b' ');
            assert_eq!(line.as_bytes()[NAME_WIDTH + 1], b' ');
        }
        let size_at = NAME_WIDTH + 2;
        let end_a = a[size_at..size_at + SIZE_WIDTH].trim();
        let end_c = c[size_at..size_at + SIZE_WIDTH].trim();
        assert!(end_a.ends_with("GB"), "{a}");
        assert!(end_c.ends_with("GB"), "{c}");
    }

    #[test]
    fn size_tiers() {
        assert_eq!(size_tier(1024 * 1024 * 1024), SizeTier::Hot);
        assert_eq!(size_tier(200 * 1024 * 1024), SizeTier::Warm);
        assert_eq!(size_tier(199 * 1024 * 1024), SizeTier::Cool);
    }
}
