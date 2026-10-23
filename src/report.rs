use crate::group::{build_tree, format_cluster, paint_line};
use crate::inventory::{human, Node};

pub fn print_scan(roots: &[Node]) {
    println!("gomi scan (read-only)\n");
    if roots.is_empty() {
        println!("no entries. use --root to set a project root.");
        return;
    }
    let tree = build_tree(roots.to_vec());
    for kind in &tree {
        println!(
            "{}",
            paint_line(
                &format_cluster(&kind.title, kind.bytes, kind.count, 0),
                kind.bytes
            )
        );
        for plugin in &kind.children {
            println!(
                "{}",
                paint_line(
                    &format_cluster(&plugin.title, plugin.bytes, plugin.count, 2),
                    plugin.bytes
                )
            );
        }
    }
    println!("\nuse TUI (default scan) to expand paths.");
}

pub fn print_inspect(n: &Node) {
    println!("gomi inspect (read-only)\n");
    print_detail(n, 0);
}

fn print_detail(n: &Node, indent: usize) {
    let pad = "  ".repeat(indent);
    println!(
        "{pad}#{}  {}  {}  {}",
        n.id,
        human(n.size_bytes),
        n.path.display(),
        n.default_action.as_str()
    );
    println!("{pad}  source: {} / {:?}", n.plugin_id, n.kind);
    println!("{pad}  {}", n.explain.what);
    println!("{pad}  if deleted: {}", n.explain.if_deleted);
    println!("{pad}  rebuild: {}", n.explain.how_rebuild);
    if !n.child_summary.is_empty() {
        println!("{pad}  safety: {}", n.child_summary);
    }
    if let Some(d) = n.git_dirty {
        println!("{pad}  git dirty: {d}");
    }
    if let Some(s) = n.stale {
        println!("{pad}  stale(>7d): {s}");
    }
    for c in &n.children {
        print_detail(c, indent + 1);
    }
}

pub fn explain_text(n: &Node) -> String {
    format!(
        "rules only, no AI\n#{}  {}  {}\n{}\nif deleted: {}\nrebuild: {}\naction: {} / {}",
        n.id,
        human(n.size_bytes),
        n.path.display(),
        n.explain.what,
        n.explain.if_deleted,
        n.explain.how_rebuild,
        n.default_action.as_str(),
        n.clean_method.as_str()
    )
}

pub fn print_explain(n: &Node) {
    println!("gomi explain\n{}", explain_text(n));
}

