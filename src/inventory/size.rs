use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Logical directory size. Permission errors count as 0.
pub fn du(path: &Path) -> u64 {
    du_with(path, &mut |_, _| {})
}

pub fn du_with(path: &Path, tick: &mut impl FnMut(&Path, u64)) -> u64 {
    let acc = Cell::new(0u64);
    let last = Cell::new(Instant::now() - Duration::from_secs(1));
    du_inner(path, &mut HashSet::new(), 0, &acc, &last, tick)
}

fn du_inner(
    path: &Path,
    seen_dev_ino: &mut HashSet<(u64, u64)>,
    depth: u32,
    acc: &Cell<u64>,
    last: &Cell<Instant>,
    tick: &mut impl FnMut(&Path, u64),
) -> u64 {
    if depth > 40 {
        return 0;
    }
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !seen_dev_ino.insert((meta.dev(), meta.ino())) {
            return 0;
        }
    }
    if meta.is_file() {
        let n = meta.len();
        acc.set(acc.get().saturating_add(n));
        maybe_tick(path, acc, last, tick);
        return n;
    }
    if !meta.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    let rd = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => return 0,
    };
    for ent in rd.flatten() {
        total = total.saturating_add(du_inner(
            &ent.path(),
            seen_dev_ino,
            depth + 1,
            acc,
            last,
            tick,
        ));
    }
    maybe_tick(path, acc, last, tick);
    total
}

fn maybe_tick(
    path: &Path,
    acc: &Cell<u64>,
    last: &Cell<Instant>,
    tick: &mut impl FnMut(&Path, u64),
) {
    if last.get().elapsed() >= Duration::from_millis(120) {
        tick(path, acc.get());
        last.set(Instant::now());
    }
}

pub fn reclaimable(selected: &[PathBuf], all_sizes: &[(PathBuf, u64)]) -> u64 {
    let mut bytes = 0u64;
    for (path, size) in all_sizes {
        if !selected.iter().any(|s| path == s || path.starts_with(s)) {
            continue;
        }
        let covered_by_ancestor = selected.iter().any(|s| path != s && path.starts_with(s));
        if covered_by_ancestor {
            continue;
        }
        bytes = bytes.saturating_add(*size);
    }
    bytes
}

pub fn human(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if b < K {
        format!("{bytes} B")
    } else if b < K * K {
        format!("{:.1} KB", b / K)
    } else if b < K * K * K {
        format!("{:.1} MB", b / (K * K))
    } else {
        format!("{:.1} GB", b / (K * K * K))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_double_count_parent_and_child() {
        let parent = PathBuf::from("/tmp/wt");
        let child = PathBuf::from("/tmp/wt/target");
        let sizes = vec![(parent.clone(), 8_000), (child.clone(), 6_000)];
        assert_eq!(reclaimable(&[parent], &sizes), 8_000);
        assert_eq!(reclaimable(&[child], &sizes), 6_000);
    }
}
