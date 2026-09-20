use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::cursor::MoveToColumn;
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};

use crate::group::{format_cluster, paint_line};
use crate::inventory::human;

pub trait ScanProgress {
    fn walking(&mut self, path: &Path, measuring: Option<u64>);
    fn sizing(&mut self, path: &Path, label: &str, running: u64, add_to_total: bool);
    fn found(&mut self, path: &Path, label: &str, bytes: u64, add_to_total: bool);
    fn finish(&mut self);
}

pub struct Quiet;

impl ScanProgress for Quiet {
    fn walking(&mut self, _path: &Path, _measuring: Option<u64>) {}
    fn sizing(&mut self, _path: &Path, _label: &str, _running: u64, _add_to_total: bool) {}
    fn found(&mut self, _path: &Path, _label: &str, _bytes: u64, _add_to_total: bool) {}
    fn finish(&mut self) {}
}

#[derive(Default, Clone, Copy)]
struct Bucket {
    bytes: u64,
    count: usize,
}

#[derive(Clone)]
struct Inflight {
    kind: String,
    plugin: String,
    bytes: u64,
    add_to_total: bool,
}

pub struct StderrProgress {
    kinds: BTreeMap<String, Bucket>,
    plugins: BTreeMap<(String, String), Bucket>,
    total: u64,
    last_walk: Instant,
    last_path: Option<std::path::PathBuf>,
    last_measuring: Option<u64>,
    inflight: Option<Inflight>,
    drawn: usize,
    tty: bool,
}

impl StderrProgress {
    pub fn new() -> Self {
        Self {
            kinds: BTreeMap::new(),
            plugins: BTreeMap::new(),
            total: 0,
            last_walk: Instant::now() - Duration::from_secs(1),
            last_path: None,
            last_measuring: None,
            inflight: None,
            drawn: 0,
            tty: io::stderr().is_terminal(),
        }
    }

    fn add_label(&mut self, label: &str, bytes: u64) {
        let (kind, plugin) = split_label(label);
        let k = self.kinds.entry(kind.clone()).or_default();
        k.bytes = k.bytes.saturating_add(bytes);
        k.count += 1;
        let p = self.plugins.entry((kind, plugin)).or_default();
        p.bytes = p.bytes.saturating_add(bytes);
        p.count += 1;
    }

    fn live_kind(&self, kind: &str) -> bool {
        self.inflight.as_ref().is_some_and(|i| i.kind == kind)
    }

    fn extra_kind(&self, kind: &str) -> u64 {
        self.inflight
            .as_ref()
            .filter(|i| i.kind == kind)
            .map(|i| i.bytes)
            .unwrap_or(0)
    }

    fn extra_plugin(&self, kind: &str, plugin: &str) -> u64 {
        self.inflight
            .as_ref()
            .filter(|i| i.kind == kind && i.plugin == plugin)
            .map(|i| i.bytes)
            .unwrap_or(0)
    }

    fn extra_total(&self) -> u64 {
        self.inflight
            .as_ref()
            .filter(|i| i.add_to_total)
            .map(|i| i.bytes)
            .unwrap_or(0)
    }

    fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for kind in ["lang", "agent", "cache", "unknown"] {
            let kb = self.kinds.get(kind).copied().unwrap_or_default();
            if kb.count == 0 && !self.live_kind(kind) {
                continue;
            }
            let kind_bytes = kb.bytes.saturating_add(self.extra_kind(kind));
            let kind_count = if kb.count == 0 { 1 } else { kb.count };
            lines.push(paint_line(
                &format_cluster(kind, kind_bytes, kind_count, 0),
                kind_bytes,
            ));
            let mut plugins: Vec<(String, Bucket)> = self
                .plugins
                .iter()
                .filter(|((k, _), _)| k == kind)
                .map(|((_, plugin), pb)| (plugin.clone(), *pb))
                .collect();
            if let Some(inf) = &self.inflight {
                if inf.kind == kind
                    && !plugins.iter().any(|(plugin, _)| plugin == &inf.plugin)
                {
                    plugins.push((inf.plugin.clone(), Bucket::default()));
                }
            }
            for (plugin, pb) in plugins {
                let plugin_bytes = pb.bytes.saturating_add(self.extra_plugin(kind, &plugin));
                let plugin_count = if pb.count == 0 { 1 } else { pb.count };
                lines.push(paint_line(
                    &format_cluster(&format!("{kind}/{plugin}"), plugin_bytes, plugin_count, 2),
                    plugin_bytes,
                ));
            }
        }
        if let Some(path) = &self.last_path {
            let grow = self
                .last_measuring
                .map(|b| format!("  sizing {:>10}", human(b)))
                .unwrap_or_default();
            let shown = truncate_path(path, 56);
            let now = format!("now   {shown}{grow}");
            if let Some(bytes) = self.last_measuring {
                lines.push(paint_line(&now, bytes));
            } else {
                lines.push(now);
            }
        }
        let total = self.total.saturating_add(self.extra_total());
        lines.push(paint_line(
            &format!("{:<24}  {:>10}", "total", human(total)),
            total,
        ));
        lines
    }

    fn redraw(&mut self) {
        let lines = self.render_lines();
        let mut err = io::stderr();
        if self.tty && self.drawn > 0 {
            let _ = execute!(err, crossterm::cursor::MoveUp(self.drawn as u16));
        }
        for line in &lines {
            if self.tty {
                let _ = execute!(err, MoveToColumn(0), Clear(ClearType::CurrentLine));
            }
            let _ = writeln!(err, "{line}");
        }
        if self.tty && self.drawn > lines.len() {
            for _ in 0..(self.drawn - lines.len()) {
                let _ = execute!(err, MoveToColumn(0), Clear(ClearType::CurrentLine));
                let _ = writeln!(err);
            }
            let extra = (self.drawn - lines.len()) as u16;
            if extra > 0 {
                let _ = execute!(err, crossterm::cursor::MoveUp(extra));
            }
        }
        self.drawn = lines.len();
        let _ = err.flush();
    }
}

fn truncate_path(path: &Path, max: usize) -> String {
    let s = path.display().to_string();
    if s.len() <= max {
        return s;
    }
    let tail = s.chars().rev().take(max.saturating_sub(1)).collect::<String>();
    format!("…{}", tail.chars().rev().collect::<String>())
}

impl ScanProgress for StderrProgress {
    fn walking(&mut self, path: &Path, measuring: Option<u64>) {
        self.last_path = Some(path.to_path_buf());
        self.last_measuring = measuring;
        if !self.tty {
            return;
        }
        if measuring.is_none() && self.last_walk.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.last_walk = Instant::now();
        self.redraw();
    }

    fn sizing(&mut self, path: &Path, label: &str, running: u64, add_to_total: bool) {
        let (kind, plugin) = split_label(label);
        self.last_path = Some(path.to_path_buf());
        self.last_measuring = Some(running);
        self.inflight = Some(Inflight {
            kind,
            plugin,
            bytes: running,
            add_to_total,
        });
        if !self.tty {
            return;
        }
        self.last_walk = Instant::now();
        self.redraw();
    }

    fn found(&mut self, path: &Path, label: &str, bytes: u64, add_to_total: bool) {
        self.inflight = None;
        self.last_path = Some(path.to_path_buf());
        self.last_measuring = None;
        self.add_label(label, bytes);
        if add_to_total {
            self.total = self.total.saturating_add(bytes);
        }
        self.redraw();
    }

    fn finish(&mut self) {
        self.inflight = None;
        self.last_path = None;
        self.last_measuring = None;
        self.redraw();
    }
}

fn split_label(label: &str) -> (String, String) {
    match label.split_once('/') {
        Some((kind, plugin)) => (kind.to_string(), plugin.to_string()),
        None => (label.to_string(), "other".to_string()),
    }
}

pub fn default_progress() -> Box<dyn ScanProgress> {
    Box::new(StderrProgress::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn live_sizing_changes_color() {
        let mut p = StderrProgress::new();
        p.tty = false;
        p.sizing(Path::new("/tmp/target"), "lang/rust", 10, true);
        let cool = p.render_lines();
        assert!(
            cool.iter()
                .any(|l| l.contains("lang/rust") && l.contains("\x1b[1;32m")),
            "{cool:?}"
        );

        p.sizing(
            Path::new("/tmp/target"),
            "lang/rust",
            200 * 1024 * 1024,
            true,
        );
        let warm = p.render_lines();
        assert!(
            warm.iter()
                .any(|l| l.contains("lang/rust") && l.contains("\x1b[1;33m")),
            "{warm:?}"
        );

        p.sizing(
            Path::new("/tmp/target"),
            "lang/rust",
            1024 * 1024 * 1024,
            true,
        );
        let hot = p.render_lines();
        assert!(
            hot.iter()
                .any(|l| l.contains("lang/rust") && l.contains("\x1b[1;31m")),
            "{hot:?}"
        );

        p.found(
            Path::new("/tmp/target"),
            "lang/rust",
            1024 * 1024 * 1024,
            true,
        );
        let committed = p.render_lines();
        let rust = committed
            .iter()
            .find(|l| l.contains("lang/rust"))
            .expect("rust row");
        assert!(rust.contains("\x1b[1;31m"), "{rust}");
        assert!(rust.contains("1.0 GB"), "{rust}");
        assert_eq!(
            committed.iter().filter(|l| l.contains("lang/rust")).count(),
            1
        );
    }
}
