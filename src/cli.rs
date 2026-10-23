use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "gomi",
    about = "Explain developer disk usage. CLI clean is dry-run. TUI can Trash marked suggest_trash items after confirm."
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
        /// Agent worktrees. Bare --agent is all; --agent codex is one plugin.
        #[arg(long, num_args = 0..=1, default_missing_value = "", value_name = "NAME")]
        agent: Vec<String>,
        /// Language artifacts. Bare --lang is all; --lang rust is one plugin.
        #[arg(long, num_args = 0..=1, default_missing_value = "", value_name = "NAME")]
        lang: Vec<String>,
        /// Toolchain caches. Bare --cache is all; --cache npm is one plugin.
        #[arg(long, num_args = 0..=1, default_missing_value = "", value_name = "NAME")]
        cache: Vec<String>,
    },
    /// Expand one path (including children).
    Inspect {
        path: PathBuf,
        #[arg(long)]
        root: Vec<PathBuf>,
        #[arg(long)]
        no_hotspots: bool,
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
