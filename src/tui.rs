use std::collections::HashSet;
use std::io::{self, stdout};
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;

use crate::engine::{apply, selectable};
use crate::group::{build_tree, size_tier, SizeTier, TreeItem};
use crate::inventory::{human, CleanMethod, Node};
use crate::report::explain_text;

pub fn run(roots: Vec<Node>, stale_after_days: u64) -> io::Result<()> {
    let tree = build_tree(roots);
    if tree.is_empty() {
        println!("no entries. use --root to set a project root.");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let mut app = App::new(tree, stale_after_days);
    let result = loop {
        terminal.draw(|f| draw(f, &mut app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match app.mode {
            Mode::Confirm => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => app.confirm_apply(),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.mode = Mode::Browse;
                    app.status = "cancelled".into();
                }
                _ => {}
            },
            Mode::Command => match key.code {
                KeyCode::Esc => {
                    app.mode = Mode::Browse;
                    app.input.clear();
                }
                KeyCode::Enter => app.submit_command(),
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char(c) => app.input.push(c),
                _ => {}
            },
            Mode::Browse => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.move_by(1),
                KeyCode::Up | KeyCode::Char('k') => app.move_by(-1),
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => app.expand_or_toggle(),
                KeyCode::Left | KeyCode::Char('h') => app.collapse(),
                KeyCode::Char(' ') => app.toggle_mark(),
                KeyCode::Char('d') => app.request_delete(),
                KeyCode::Char(':') => {
                    app.mode = Mode::Command;
                    app.input.clear();
                    app.explain = None;
                }
                _ => {}
            },
        }
    };
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Browse,
    Confirm,
    Command,
}

struct App {
    roots: Vec<TreeItem>,
    expanded: HashSet<String>,
    marked: HashSet<String>,
    cursor: usize,
    list_state: ListState,
    mode: Mode,
    status: String,
    input: String,
    explain: Option<String>,
    stale_after_days: u64,
}

impl App {
    fn new(roots: Vec<TreeItem>, stale_after_days: u64) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            roots,
            expanded: HashSet::new(),
            marked: HashSet::new(),
            cursor: 0,
            list_state,
            mode: Mode::Browse,
            status: String::new(),
            input: String::new(),
            explain: None,
            stale_after_days,
        }
    }

    fn visible(&self) -> Vec<(&TreeItem, usize)> {
        let mut out = Vec::new();
        for n in &self.roots {
            collect_visible(n, 0, &self.expanded, &mut out);
        }
        out
    }

    fn selected(&self) -> Option<&TreeItem> {
        self.visible().get(self.cursor).map(|(n, _)| *n)
    }

    fn move_by(&mut self, delta: isize) {
        let len = self.visible().len();
        if len == 0 {
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, len as isize - 1) as usize;
        self.list_state.select(Some(self.cursor));
        self.explain = None;
    }

    fn expand_or_toggle(&mut self) {
        let Some((id, has_children, open)) = self.cursor_state() else {
            return;
        };
        if !has_children {
            return;
        }
        if open {
            self.expanded.remove(&id);
        } else {
            self.expanded.insert(id);
        }
    }

    fn collapse(&mut self) {
        let Some((id, has_children, open)) = self.cursor_state() else {
            return;
        };
        if has_children && open {
            self.expanded.remove(&id);
        }
    }

    fn cursor_state(&self) -> Option<(String, bool, bool)> {
        let vis = self.visible();
        let (n, _) = vis.get(self.cursor)?;
        let open = self.expanded.contains(&n.id);
        Some((n.id.clone(), !n.children.is_empty(), open))
    }

    fn toggle_mark(&mut self) {
        let ids = {
            let Some(item) = self.selected() else {
                return;
            };
            eligible_ids(item)
        };
        if ids.is_empty() {
            self.status = "not suggest_trash; not marked".into();
            return;
        }
        let all_marked = ids.iter().all(|id| self.marked.contains(id));
        if all_marked {
            for id in &ids {
                self.marked.remove(id);
            }
            self.status = format!("unmarked {}", ids.len());
        } else {
            for id in &ids {
                self.marked.insert(id.clone());
            }
            self.status = format!("marked {}", ids.len());
        }
    }

    fn request_delete(&mut self) {
        let mut ids: Vec<String> = self.marked.iter().cloned().collect();
        if ids.is_empty() {
            ids = self
                .selected()
                .map(eligible_ids)
                .unwrap_or_default();
            for id in &ids {
                self.marked.insert(id.clone());
            }
        }
        let pending = self.pending_nodes(&ids);
        if pending.is_empty() {
            self.status = "nothing marked that is suggest_trash".into();
            return;
        }
        self.mode = Mode::Confirm;
        self.status = format!(
            "confirm {} item(s) ({})",
            pending.len(),
            human(pending.iter().map(|n| n.size_bytes).sum())
        );
    }

    fn confirm_apply(&mut self) {
        let ids: Vec<String> = self.marked.iter().cloned().collect();
        let pending = self.pending_nodes(&ids);
        if pending.is_empty() {
            self.mode = Mode::Browse;
            self.status = "nothing to apply".into();
            return;
        }
        let refs: Vec<&Node> = pending.iter().collect();
        let report = apply(&refs, self.stale_after_days);
        let deleted_paths: Vec<_> = report.done.iter().map(|d| d.path.clone()).collect();
        remove_paths(&mut self.roots, &deleted_paths);
        recount(&mut self.roots);
        for done in &report.done {
            self.marked.remove(&done.id);
        }
        self.mode = Mode::Browse;
        self.status = report.summary();
        let len = self.visible().len();
        if len == 0 {
            self.cursor = 0;
            self.list_state.select(None);
        } else {
            self.cursor = self.cursor.min(len - 1);
            self.list_state.select(Some(self.cursor));
        }
    }

    fn submit_command(&mut self) {
        let line = self.input.clone();
        self.mode = Mode::Browse;
        self.input.clear();
        match parse_command(&line) {
            Ok(ExplainQuery::Selected) => match self.selected().and_then(|item| item.node.clone()) {
                Some(node) => {
                    self.status = format!("explain #{}", node.id);
                    self.explain = Some(explain_text(&node));
                }
                None => {
                    self.explain = None;
                    self.status = "select a path, or: explain <path|id>".into();
                }
            },
            Ok(ExplainQuery::Target(arg)) => match find_explain_target(&self.roots, &arg) {
                Ok(node) => {
                    self.status = format!("explain #{}", node.id);
                    self.explain = Some(explain_text(&node));
                }
                Err(err) => {
                    self.explain = None;
                    self.status = err;
                }
            },
            Err(err) => {
                self.explain = None;
                self.status = err;
            }
        }
    }

    fn pending_nodes(&self, ids: &[String]) -> Vec<Node> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(n) = find_node(&self.roots, id) {
                if selectable(n) {
                    out.push(n.clone());
                }
            }
        }
        out
    }
}

fn eligible_ids(item: &TreeItem) -> Vec<String> {
    let mut out = Vec::new();
    collect_eligible(item, &mut out);
    out
}

fn collect_eligible(item: &TreeItem, out: &mut Vec<String>) {
    if let Some(n) = &item.node {
        if selectable(n) {
            out.push(n.id.clone());
        }
    }
    for c in &item.children {
        collect_eligible(c, out);
    }
}

fn find_node<'a>(items: &'a [TreeItem], id: &str) -> Option<&'a Node> {
    for item in items {
        if let Some(n) = &item.node {
            if n.id == id {
                return Some(n);
            }
        }
        if let Some(n) = find_node(&item.children, id) {
            return Some(n);
        }
    }
    None
}

fn path_covered(path: &Path, deleted: &[std::path::PathBuf]) -> bool {
    deleted
        .iter()
        .any(|d| path == d || path.starts_with(d))
}

fn remove_paths(items: &mut Vec<TreeItem>, deleted: &[std::path::PathBuf]) {
    items.retain(|item| match &item.node {
        Some(n) => !path_covered(&n.path, deleted),
        None => true,
    });
    for item in items.iter_mut() {
        remove_paths(&mut item.children, deleted);
    }
    items.retain(|item| item.node.is_some() || !item.children.is_empty());
}

fn recount(items: &mut [TreeItem]) {
    for item in items.iter_mut() {
        recount(&mut item.children);
        if item.node.is_none() {
            item.bytes = item.children.iter().map(|c| c.bytes).sum();
            item.count = item
                .children
                .iter()
                .map(|c| if c.node.is_some() { 1 } else { c.count })
                .sum();
        }
    }
}

fn collect_visible<'a>(
    n: &'a TreeItem,
    depth: usize,
    expanded: &HashSet<String>,
    out: &mut Vec<(&'a TreeItem, usize)>,
) {
    out.push((n, depth));
    if expanded.contains(&n.id) {
        for c in &n.children {
            collect_visible(c, depth + 1, expanded, out);
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &mut App) {
    let mut constraints = vec![
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(8),
    ];
    if app.mode == Mode::Command {
        constraints.push(Constraint::Length(3));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());

    let header = match app.mode {
        Mode::Confirm => format!(
            "CONFIRM  y trash marked  n cancel  |  {}  |  never uses rm",
            app.status
        ),
        Mode::Command => "command  type explain [path|id]  enter run  esc cancel".into(),
        Mode::Browse => {
            let marks = app.marked.len();
            let extra = if app.status.is_empty() {
                String::new()
            } else {
                format!("  |  {}", app.status)
            };
            format!(
                "gomi  : explain  space mark  d trash  j/k move  q quit  marked:{marks}{extra}"
            )
        }
    };
    let header_style = if app.mode == Mode::Confirm {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let header = Paragraph::new(header)
        .style(header_style)
        .block(Block::default().borders(Borders::ALL).title("scan"));
    frame.render_widget(header, chunks[0]);

    let vis = app.visible();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|(n, depth)| {
            let mark = if n.children.is_empty() {
                " "
            } else if app.expanded.contains(&n.id) {
                "v"
            } else {
                ">"
            };
            let box_mark = mark_box(n, &app.marked);
            let indent = "  ".repeat(*depth);
            let count = if n.node.is_none() {
                format!("{:>5}", n.count)
            } else {
                "     ".into()
            };
            let name = format!("{box_mark} {indent}{mark} {}", n.title);
            let line = format!("{name:<30}  {:>10}  {count}", human(n.bytes));
            let color = match size_tier(n.bytes) {
                SizeTier::Hot => Color::Red,
                SizeTier::Warm => Color::Yellow,
                SizeTier::Cool => Color::Green,
            };
            ListItem::new(Line::from(Span::styled(line, Style::default().fg(color))))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("tree"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, chunks[1], &mut app.list_state);

    let (detail, detail_title) = match app.mode {
        Mode::Confirm => (confirm_detail(app), "detail"),
        Mode::Command | Mode::Browse => {
            if let Some(text) = &app.explain {
                (text.clone(), "explain")
            } else {
                (browse_detail(app), "detail")
            }
        }
    };
    let pane = Paragraph::new(detail)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(detail_title));
    frame.render_widget(pane, chunks[2]);

    if app.mode == Mode::Command {
        let shown = if app.input.is_empty() {
            "> ".to_string()
        } else {
            format!("> {}", app.input)
        };
        let prompt = Paragraph::new(shown)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("command: explain [path|id]"),
            );
        frame.render_widget(prompt, chunks[3]);
        let cursor_x = chunks[3]
            .x
            .saturating_add(3)
            .saturating_add(app.input.chars().count() as u16);
        let max_x = chunks[3].x.saturating_add(chunks[3].width.saturating_sub(2));
        frame.set_cursor_position((cursor_x.min(max_x), chunks[3].y + 1));
    }
}

fn mark_box(item: &TreeItem, marked: &HashSet<String>) -> &'static str {
    let ids = eligible_ids(item);
    if ids.is_empty() {
        return "   ";
    }
    if ids.iter().all(|id| marked.contains(id)) {
        "[x]"
    } else if ids.iter().any(|id| marked.contains(id)) {
        "[~]"
    } else {
        "[ ]"
    }
}

fn browse_detail(app: &App) -> String {
    match app.selected() {
        Some(n) => match &n.node {
            Some(node) => {
                let mut lines = vec![
                    node.path.display().to_string(),
                    format!("#{}", node.id),
                    node.explain.what.clone(),
                ];
                if !node.child_summary.is_empty() {
                    lines.push(format!("safety: {}", node.child_summary));
                }
                lines.push(format!("if deleted: {}", node.explain.if_deleted));
                lines.push(format!("rebuild: {}", node.explain.how_rebuild));
                lines.push(format!(
                    "action: {} / {}",
                    node.default_action.as_str(),
                    node.clean_method.as_str()
                ));
                if selectable(node) {
                    lines.push("space marks this; d then y moves it to Trash.".into());
                } else {
                    lines.push("not suggest_trash; TUI will not delete this.".into());
                }
                lines.join("\n")
            }
            None => format!(
                "{}  {}  {} item(s)\nenter to expand; space marks suggest_trash children",
                n.title,
                human(n.bytes),
                n.count
            ),
        },
        None => String::new(),
    }
}

fn confirm_detail(app: &App) -> String {
    let ids: Vec<String> = app.marked.iter().cloned().collect();
    let pending = app.pending_nodes(&ids);
    let mut lines = vec![
        "Move marked suggest_trash items. Never uses rm.".into(),
        "y confirm   n/esc cancel".into(),
        String::new(),
    ];
    for n in pending.iter().take(6) {
        let how = match n.clean_method {
            CleanMethod::GitWorktreeRemove => "git worktree remove",
            _ => "Trash",
        };
        lines.push(format!(
            "{}  {}  {}",
            how,
            human(n.size_bytes),
            n.path.display()
        ));
    }
    if pending.len() > 6 {
        lines.push(format!("... {} more", pending.len() - 6));
    }
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExplainQuery {
    Selected,
    Target(String),
}

fn parse_command(line: &str) -> Result<ExplainQuery, String> {
    let line = line.trim();
    let Some(rest) = line.strip_prefix("explain") else {
        return Err("unknown command. try: explain [path|id]".into());
    };
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return Err("unknown command. try: explain [path|id]".into());
    }
    let arg = rest.trim().trim_start_matches('#');
    if arg.is_empty() {
        Ok(ExplainQuery::Selected)
    } else {
        Ok(ExplainQuery::Target(arg.to_string()))
    }
}

fn find_explain_target(roots: &[TreeItem], arg: &str) -> Result<Node, String> {
    let mut nodes = Vec::new();
    collect_nodes(roots, &mut nodes);
    let mut seen = HashSet::new();
    nodes.retain(|n| seen.insert(n.id.clone()));
    if let Some(n) = nodes.iter().find(|n| n.id == arg) {
        return Ok((*n).clone());
    }
    let path = Path::new(arg);
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let exact: Vec<_> = nodes
        .iter()
        .filter(|n| n.path == path || n.path == canon)
        .collect();
    if exact.len() == 1 {
        return Ok((*exact[0]).clone());
    }
    let suffix: Vec<_> = nodes
        .iter()
        .filter(|n| path_tail(&n.path, arg))
        .collect();
    match suffix.len() {
        1 => Ok((*suffix[0]).clone()),
        0 => Err(format!("not found: {arg}")),
        _ => Err(format!("ambiguous: {arg}")),
    }
}

fn path_tail(path: &Path, arg: &str) -> bool {
    let shown = path.display().to_string();
    shown == arg || shown.ends_with(&format!("/{arg}"))
}

fn collect_nodes<'a>(items: &'a [TreeItem], out: &mut Vec<&'a Node>) {
    for item in items {
        if let Some(n) = &item.node {
            out.push(n);
        }
        collect_nodes(&item.children, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explain_command_parse() {
        assert_eq!(parse_command("explain").unwrap(), ExplainQuery::Selected);
        assert_eq!(
            parse_command("explain /tmp/target").unwrap(),
            ExplainQuery::Target("/tmp/target".into())
        );
        assert_eq!(
            parse_command("explain #abc").unwrap(),
            ExplainQuery::Target("abc".into())
        );
        assert!(parse_command("inspect").is_err());
        assert!(parse_command("explained").is_err());
    }
}
