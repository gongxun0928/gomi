use std::io::IsTerminal;

use clap::Parser;

use gomi::cli::{Cli, Command};
use gomi::config::Config;
use gomi::engine::{dry_run, scan};
use gomi::inventory::{Kind, Node};
use gomi::report;

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Command::Scan {
            root,
            no_hotspots,
            plain,
        } => {
            let cfg = Config::load(&root, !no_hotspots);
            let inv = scan(&cfg);
            let tty = std::io::stdout().is_terminal();
            if plain || !tty {
                report::print_scan(&inv.roots);
            } else if let Err(err) = gomi::tui::run(inv.roots) {
                eprintln!("tui failed: {err}");
                std::process::exit(1);
            }
        }
        Command::Inspect { path, root, no_hotspots } => {
            let cfg = Config::load(&root, !no_hotspots);
            let inv = scan(&cfg);
            let canon = std::fs::canonicalize(&path).unwrap_or(path);
            match inv.find_by_path(&canon).or_else(|| closest(&inv.roots, &canon)) {
                Some(n) => report::print_inspect(n),
                None => {
                    eprintln!("no matching node: {}", canon.display());
                    std::process::exit(1);
                }
            }
        }
        Command::Worktrees { root } => {
            let cfg = Config::load(&root, true);
            let inv = scan(&cfg);
            let trees: Vec<&Node> = inv
                .roots
                .iter()
                .filter(|n| n.kind == Kind::AgentWorkspace)
                .collect();
            if trees.is_empty() {
                println!("no agent / git worktrees found");
            } else {
                report::print_scan(&trees.into_iter().cloned().collect::<Vec<_>>());
            }
        }
        Command::Clean { ids, root, no_hotspots } => {
            let cfg = Config::load(&root, !no_hotspots);
            let inv = scan(&cfg);
            let picked: Vec<&Node> = if ids.is_empty() {
                inv.roots.iter().flat_map(|n| n.walk_flat()).collect()
            } else {
                ids.iter().filter_map(|id| inv.find(id)).collect()
            };
            println!("{}", dry_run(&picked));
        }
        Command::Explain { id_or_path, root, no_hotspots } => {
            let cfg = Config::load(&root, !no_hotspots);
            let inv = scan(&cfg);
            let n = inv.find(&id_or_path).or_else(|| {
                let p = std::path::PathBuf::from(&id_or_path);
                let canon = std::fs::canonicalize(&p).unwrap_or(p);
                inv.find_by_path(&canon)
            });
            match n {
                Some(n) => report::print_explain(n),
                None => {
                    eprintln!("not found: {id_or_path}");
                    std::process::exit(1);
                }
            }
        }
    }
}

fn closest<'a>(roots: &'a [Node], path: &std::path::Path) -> Option<&'a Node> {
    roots
        .iter()
        .filter(|n| path.starts_with(&n.path) || n.path.starts_with(path))
        .max_by_key(|n| n.path.as_os_str().len())
}
