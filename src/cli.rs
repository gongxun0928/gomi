use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "gomi",
    about = "Explain developer disk usage. Scan and dry-run only; does not delete."
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Scan project roots and hotspots. Read-only.
    Scan {
        #[arg(long)]
        root: Vec<PathBuf>,
        /// Skip agent worktrees and toolchain caches.
        #[arg(long)]
        no_hotspots: bool,
        /// Print a flat list instead of the tree TUI.
        #[arg(long)]
        plain: bool,
    },
    /// Expand one path (including children).
    Inspect {
        path: PathBuf,
        #[arg(long)]
        root: Vec<PathBuf>,
        #[arg(long)]
        no_hotspots: bool,
    },
    /// List agent / git worktrees.
    Worktrees {
        #[arg(long)]
        root: Vec<PathBuf>,
    },
    /// Print what would happen. Never deletes.
    Clean {
        ids: Vec<String>,
        #[arg(long)]
        root: Vec<PathBuf>,
        #[arg(long)]
        no_hotspots: bool,
    },
    /// Rule-based explanation. No model call.
    Explain {
        id_or_path: String,
        #[arg(long)]
        root: Vec<PathBuf>,
        #[arg(long)]
        no_hotspots: bool,
    },
}
