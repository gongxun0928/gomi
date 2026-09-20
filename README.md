# gomi

CLI for explaining developer disk usage: build artifacts, agent worktrees, toolchain caches.

**This version only scans and dry-runs. It does not delete, does not use Trash, and has no `--apply`.**

```bash
cargo run -- scan --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- scan --plain --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- inspect ~/workspace/code/github/curvine-max/target --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- explain <id> --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- clean --root ~/workspace/code/github/curvine-max --no-hotspots
cargo run -- worktrees
```

On a TTY, `scan` opens a collapsed tree: `j`/`k` move, Enter expands children, `q` quits. `--plain` or a pipe prints the old list.

- With no `--root`, scan cwd and `~/workspace`.
- `--no-hotspots` skips agent worktrees and toolchain caches.
- `clean` is always dry-run.
