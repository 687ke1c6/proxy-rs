use std::path::{Path, PathBuf};

/// Resolves `target_dir` (an arbitrary `/`-separated relative path; `..`/empty
/// segments rejected) under `root` (already canonical), creating it if needed and
/// rejecting any traversal outside of `root` — including via symlinks, since the
/// joined directory is canonicalized before the check.
///
/// Shared by any protocol that lets a client pick a destination directory within a
/// configured volume (file-send, rsync).
pub fn safe_join_dir(root: &Path, target_dir: &str) -> Result<PathBuf, String> {
    let mut dir = root.to_path_buf();
    for component in target_dir.split('/').filter(|s| !s.is_empty()) {
        if component == "." || component == ".." {
            return Err(format!("invalid target path segment: {component:?}"));
        }
        dir.push(component);
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create target directory: {e}"))?;
    let canonical_dir = dir.canonicalize().map_err(|e| format!("failed to resolve target directory: {e}"))?;
    if !canonical_dir.starts_with(root) {
        return Err("target path escapes the volume root".to_string());
    }

    Ok(canonical_dir)
}

/// Parses a `volume[/dir]` spec (e.g. an rsync destination-path token) into
/// `(volume, target_dir)`. The volume name is required and must be non-empty — unlike
/// file-send's `--target`, there's no "omit it, default to the sole volume" case here,
/// since rsync's own syntax always requires some non-empty destination-path string.
pub fn parse_volume_path(spec: &str) -> Result<(String, String), String> {
    let (volume, rest) = spec.split_once('/').unwrap_or((spec, ""));
    if volume.is_empty() {
        return Err(format!("invalid destination {spec:?}: must start with a volume name"));
    }
    let target_dir = rest.trim_end_matches('/').to_string();
    Ok((volume.to_string(), target_dir))
}
