# gomi

CLI for explaining developer disk usage: build artifacts, agent worktrees, toolchain caches.

**`gomi clean` is still dry-run and has no `--apply`.** In the scan TUI, `space` marks `suggest_trash` items and `d` then `y` moves them to Trash (linked worktrees use `git worktree remove`). It never `rm`s.

```bash
cargo run -- scan --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- scan --agent
cargo run -- scan --agent codex
cargo run -- scan --lang rust
cargo run -- scan --plain --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- inspect ~/workspace/code/github/curvine-max/target --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- explain <id> --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- clean --root ~/workspace/code/github/curvine-max --no-hotspots
```

On a TTY, `scan` opens a collapsed tree: `j`/`k` move, Enter expands, `space` marks, `d` then `y` trashes, `:` then `explain` for the selected path, `q` quits. `--plain` or a pipe prints the old list.

- With no `--root`, scan cwd and `~/workspace`.
- `--no-hotspots` skips agent worktrees and toolchain caches.
- `--agent` / `--lang` / `--cache` limit the scan. Bare flag is the whole kind; a name picks one plugin (`--agent codex`, `--lang rust`). Combine them.
- `clean` is always dry-run.
