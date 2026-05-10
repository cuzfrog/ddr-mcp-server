use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub(crate) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub(crate) fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += dir_size(&path);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Canonicalize a path. If it points to a file, return the parent directory.
pub(crate) fn resolve_input_root(path: &Path) -> anyhow::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    if canonical.is_file() {
        canonical
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Cannot determine parent of {}", canonical.display()))
    } else {
        Ok(canonical)
    }
}

/// Canonicalize a path, returning a user-facing error if it does not exist.
pub(crate) fn resolve_repo_root(path: &Path) -> anyhow::Result<PathBuf> {
    path.canonicalize()
        .map_err(|_| anyhow::anyhow!("path '{}' does not exist", path.display()))
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_input_root_with_file_returns_parent() {
        let base = std::env::temp_dir().join("docent_test_fs_file_parent");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let file_path = base.join("test.md");
        std::fs::write(&file_path, "content").unwrap();
        let root = resolve_input_root(&file_path).unwrap();
        assert_eq!(root, base.canonicalize().unwrap());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_input_root_with_directory_returns_self() {
        let base = std::env::temp_dir().join("docent_test_fs_dir_self");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let canonical_base = base.canonicalize().unwrap();
        let root = resolve_input_root(&base).unwrap();
        assert_eq!(root, canonical_base);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_input_root_nonexistent_path_returns_error() {
        let result = resolve_input_root(Path::new("/nonexistent/path/for/sure"));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_repo_root_existing_path_succeeds() {
        let base = std::env::temp_dir().join("docent_test_fs_repo_exists");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let canonical = base.canonicalize().unwrap();
        let result = resolve_repo_root(&base).unwrap();
        assert_eq!(result, canonical);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_repo_root_nonexistent_path_returns_error() {
        let result = resolve_repo_root(Path::new("/nonexistent/repo/path"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not exist"));
    }
}
