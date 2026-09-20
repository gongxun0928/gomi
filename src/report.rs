use crate::inventory::{human, Kind, Node};

pub fn print_scan(roots: &[Node]) {
    println!("gomi scan (read-only)\n");
    if roots.is_empty() {
        println!("no entries. use --root to set a project root.");
        return;
    }
    let mut total = 0u64;
    for n in roots {
        total = total.saturating_add(n.size_bytes);
        print_root(n);
    }
    println!(
        "\nroot total {} (child sizes are included in parents; not added twice)",
        human(total)
    );
    println!("inspect: gomi inspect <path>");
    println!("explain: gomi explain <id>");
    println!("dry-run: gomi clean [id...]");
}

fn print_root(n: &Node) {
    let kind = kind_label(n.kind);
    let extra = if n.child_summary.is_empty() {
        String::new()
    } else {
        format!("  {}", n.child_summary)
    };
    let dirty = match n.git_dirty {
        Some(true) => "  dirty",
        Some(false) => "  clean",
        None => "",
    };
    let stale = match n.stale {
        Some(true) => "  stale",
        Some(false) => "  recent",
        None => "",
    };
    println!(
        "#{}  {:>10}  {:<16}  {}{}{}{}  [{}]",
        n.id,
        human(n.size_bytes),
        format!("{}/{}", kind, n.plugin_id),
        n.path.display(),
        extra,
        dirty,
        stale,
        n.default_action.as_str()
    );
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
    if let Some(d) = n.git_dirty {
        println!("{pad}  git dirty: {d}");
    }
    if let Some(s) = n.stale {
        println!("{pad}  stale(>14d): {s}");
    }
    for c in &n.children {
        print_detail(c, indent + 1);
    }
}

pub fn print_explain(n: &Node) {
    println!("gomi explain (rules only, no AI)\n");
    println!("#{}  {}", n.id, n.path.display());
    println!("{}", n.explain.what);
    println!("if deleted: {}", n.explain.if_deleted);
    println!("rebuild: {}", n.explain.how_rebuild);
    println!("action: {} / {}", n.default_action.as_str(), n.clean_method.as_str());
}

fn kind_label(k: Kind) -> &'static str {
    match k {
        Kind::LanguageArtifact => "lang",
        Kind::AgentWorkspace => "agent",
        Kind::Cache => "cache",
        Kind::Unknown => "unknown",
    }
}
