use sha1::{Digest, Sha1};
use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn default_game_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(env::temp_dir)
        .join(".minecraft")
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".local").join("share")))
        .unwrap_or_else(env::temp_dir)
        .join("Flint")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn resolve_game_dir(value: &str) -> PathBuf {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return absolute_path(default_game_dir());
    }

    if trimmed == "~" {
        return absolute_path(dirs::home_dir().unwrap_or_else(env::temp_dir));
    }
    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return absolute_path(home.join(rest));
        }
    }

    absolute_path(PathBuf::from(trimmed))
}

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .map(|working_directory| working_directory.join(&path))
            .unwrap_or(path)
    }
}

pub fn normalize_relative_metadata_path(value: &str) -> Option<String> {
    let normalized = value.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') {
        return None;
    }

    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return None;
        }
        parts.push(part);
    }
    Some(parts.join("/"))
}

pub fn is_safe_relative_path(value: &str) -> bool {
    normalize_relative_metadata_path(value).is_some()
}

pub fn path_dedup_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        text.to_ascii_lowercase()
    } else {
        text
    }
}

#[cfg(test)]
pub fn sha1_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sha1_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| {
        format!(
            "Cannot open file for SHA-1 verification ({}): {error}",
            path.display()
        )
    })?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            format!(
                "Cannot read file for SHA-1 verification ({}): {error}",
                path.display()
            )
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn is_sha1_hex(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn component_is_safe(value: &str) -> bool {
    if value.is_empty() || value == "." || value == ".." || value.len() > 128 {
        return false;
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
}

pub fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    let normalized = normalize_relative_metadata_path(relative)?;
    Some(base.join(normalized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_metadata_paths_reject_traversal() {
        assert_eq!(
            normalize_relative_metadata_path("net/example/lib.jar"),
            Some("net/example/lib.jar".to_string())
        );
        assert!(!is_safe_relative_path("../outside.jar"));
        assert!(!is_safe_relative_path("a/../../outside.jar"));
        assert!(!is_safe_relative_path("/absolute.jar"));
        assert!(!is_safe_relative_path("C:/absolute.jar"));
        assert!(!is_safe_relative_path("a\\..\\outside.jar"));
    }

    #[test]
    fn sha1_helper_matches_known_vector() {
        assert_eq!(
            sha1_bytes(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn relative_game_dirs_resolve_to_absolute_paths() {
        let resolved = resolve_game_dir("relative-game-dir");
        assert!(resolved.is_absolute());
        assert!(resolved.ends_with("relative-game-dir"));
    }

    #[test]
    fn component_validation_rejects_separators() {
        assert!(component_is_safe("1.20.1-forge-47.2.0"));
        assert!(!component_is_safe(".."));
        assert!(!component_is_safe("../escape"));
        assert!(!component_is_safe("a/b"));
    }
}
