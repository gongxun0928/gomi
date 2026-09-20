use std::collections::HashSet;
use std::io::{self, stdout};

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

use crate::group::{build_tree, size_tier, SizeTier, TreeItem};
use crate::inventory::{human, Node};

pub fn run(roots: Vec<Node>) -> io::Result<()> {
    let tree = build_tree(roots);
    if tree.is_empty() {
        println!("no entries. use --root to set a project root.");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let mut app = App::new(tree);
    let result = loop {
        terminal.draw(|f| draw(f, &mut app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
            KeyCode::Down | KeyCode::Char('j') => app.move_by(1),
            KeyCode::Up | KeyCode::Char('k') => app.move_by(-1),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => app.expand_or_toggle(),
            KeyCode::Left | KeyCode::Char('h') => app.collapse(),
            _ => {}
        }
    };
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    result
}

struct App {
    roots: Vec<TreeItem>,
    expanded: HashSet<String>,
    cursor: usize,
    list_state: ListState,
}

impl App {
    fn new(roots: Vec<TreeItem>) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            roots,
            expanded: HashSet::new(),
            cursor: 0,
            list_state,
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(8),
        ])
        .split(frame.area());

    let header = Paragraph::new("gomi  read-only  j/k move  enter expand  h collapse  q quit")
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
            let indent = "  ".repeat(*depth);
            let count = if n.node.is_none() {
                format!("{:>5}", n.count)
            } else {
                "     ".into()
            };
            let name = format!("{indent}{mark} {}", n.title);
            let line = format!("{name:<26}  {:>10}  {count}", human(n.bytes));
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

    let detail = match app.selected() {
        Some(n) => match &n.node {
            Some(node) => format!(
                "{}\n#{}\n{}\nif deleted: {}\nrebuild: {}\naction: {} / {}",
                node.path.display(),
                node.id,
                node.explain.what,
                node.explain.if_deleted,
                node.explain.how_rebuild,
                node.default_action.as_str(),
                node.clean_method.as_str()
            ),
            None => format!(
                "{}  {}  {} item(s)\nenter to expand",
                n.title,
                human(n.bytes),
                n.count
            ),
        },
        None => String::new(),
    };
    let pane = Paragraph::new(detail)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("detail"));
    frame.render_widget(pane, chunks[2]);
}
