use std::path::Path;

/// Stable short id: first 12 hex chars of blake3(canonical path).
pub fn path_id(path: &Path) -> String {
    let canon = std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .trim_end_matches('/')
        .to_string();
    let hash = blake3::hash(canon.as_bytes());
    hash.to_hex()[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn same_path_same_id() {
        let p = PathBuf::from("/tmp/gomi-id-test");
        assert_eq!(path_id(&p), path_id(&p));
    }
}
