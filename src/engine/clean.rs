use crate::inventory::{CleanMethod, DefaultAction, Node};

/// Print intended actions. Never deletes, never calls git worktree remove, never uses Trash.
pub fn dry_run(nodes: &[&Node]) -> String {
    let mut lines = vec![
        "[dry-run] gomi will not delete files or move anything to Trash.".to_string(),
        "no --apply; official cleanup commands are not executed.".to_string(),
        String::new(),
    ];
    if nodes.is_empty() {
        lines.push("no matching entries.".into());
        return lines.join("\n");
    }
    for n in nodes {
        let verdict = match n.default_action {
            DefaultAction::SuggestTrash => match n.clean_method {
                CleanMethod::Trash => "if apply is authorized: move to Trash",
                CleanMethod::GitWorktreeRemove => "if apply is authorized: git worktree remove",
                CleanMethod::OfficialCmd => "if apply is authorized: print official command only",
                CleanMethod::None => "will not delete even if authorized",
            },
            DefaultAction::InspectOnly => "scan does not suggest delete; inspect first",
            DefaultAction::WarnOnly => "dirty or not stale; warn only",
            DefaultAction::None => "blocked by safety policy",
        };
        lines.push(format!(
            "#{}  {:<18}  {:>10}  {}  [{} / {}]",
            n.id,
            n.plugin_id,
            crate::inventory::human(n.size_bytes),
            n.path.display(),
            n.default_action.as_str(),
            verdict
        ));
        if !n.explain.what.is_empty() {
            lines.push(format!("         {}", n.explain.what));
        }
    }
    lines.join("\n")
}
