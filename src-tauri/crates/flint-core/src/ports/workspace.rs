//! Safe workspace, path, and archive-policy port contracts.

use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{net, paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub max_entries: usize,
    pub max_path_bytes: usize,
    pub max_uncompressed_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            max_path_bytes: 4_096,
            max_uncompressed_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub relative_path: String,
    pub uncompressed_size: u64,
    pub is_directory: bool,
    pub is_symlink: bool,
}

impl ArchiveEntry {
    pub fn new(
        relative_path: impl Into<String>,
        uncompressed_size: u64,
        is_directory: bool,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            uncompressed_size,
            is_directory,
            is_symlink: false,
        }
    }

    pub fn symlink(relative_path: impl Into<String>, uncompressed_size: u64) -> Self {
        Self {
            relative_path: relative_path.into(),
            uncompressed_size,
            is_directory: false,
            is_symlink: true,
        }
    }
}

fn validate_portable_component(component: &str) -> Result<(), String> {
    if component.ends_with('.') || component.ends_with(' ') {
        return Err("component must not end with a dot or space".to_string());
    }
    if component
        .chars()
        .any(|character| character.is_control() || matches!(character, '?' | '*' | '"' | '<' | '>' | '|' | ':'))
    {
        return Err("component contains a control or Windows-invalid character".to_string());
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .trim_end_matches(|character| character == ' ' || character == '.');
    if is_reserved_windows_name(stem) {
        return Err(format!("component '{component}' is a reserved Windows device name"));
    }
    Ok(())
}

fn is_reserved_windows_name(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
    ) {
        return true;
    }
    upper.len() == 4
        && (upper.starts_with("COM") || upper.starts_with("LPT"))
        && upper.as_bytes()[3].is_ascii_digit()
        && upper.as_bytes()[3] != b'0'
}

fn normalize_portable_relative_path(value: &str) -> Result<String, String> {
    let normalized = paths::normalize_relative_metadata_path(value)
        .ok_or_else(|| format!("Unsafe relative path: {value}"))?;
    for component in normalized.split('/') {
        validate_portable_component(component).map_err(|reason| {
            format!("Unsafe path component '{component}' in '{value}': {reason}")
        })?;
    }
    Ok(normalized)
}

fn portable_path_key(normalized: &str) -> String {
    normalized
        .split('/')
        .map(|component| component.trim_end_matches(|character| character == '.' || character == ' ').to_lowercase())
        .collect::<Vec<_>>()
        .join("/")
}

/// Object-safe workspace contract for safe paths, atomic writes, and archive validation.
pub trait PathPort: Send + Sync {
    fn safe_join(&self, base: &Path, relative: &str) -> Result<PathBuf, String>;
    fn ensure_directory(&self, path: &Path) -> Result<(), String>;
    fn write_atomic(&self, destination: &Path, bytes: &[u8]) -> Result<(), String>;
    fn validate_archive_entries(
        &self,
        entries: &[ArchiveEntry],
        limits: &ArchiveLimits,
    ) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SafeWorkspace;

impl PathPort for SafeWorkspace {
    fn safe_join(&self, base: &Path, relative: &str) -> Result<PathBuf, String> {
        let normalized = normalize_portable_relative_path(relative)?;
        paths::safe_join(base, &normalized)
            .ok_or_else(|| format!("Unsafe relative path: {relative}"))
    }

    fn ensure_directory(&self, path: &Path) -> Result<(), String> {
        net::ensure_directory(path)
    }

    fn write_atomic(&self, destination: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        net::ensure_directory(parent)?;
        let temp = net::temp_path_near(destination, "tmp");
        let result = (|| -> Result<(), String> {
            let mut file = fs::File::create(&temp)
                .map_err(|error| format!("Cannot create atomic write temp file: {error}"))?;
            file.write_all(bytes)
                .map_err(|error| format!("Cannot write atomic write temp file: {error}"))?;
            file.flush()
                .map_err(|error| format!("Cannot flush atomic write temp file: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("Cannot sync atomic write temp file: {error}"))?;
            drop(file);
            replace_file(&temp, destination)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }

    fn validate_archive_entries(
        &self,
        entries: &[ArchiveEntry],
        limits: &ArchiveLimits,
    ) -> Result<(), String> {
        if entries.len() > limits.max_entries {
            return Err(format!(
                "Archive contains {} entries, limit {}.",
                entries.len(),
                limits.max_entries
            ));
        }
        let mut total_bytes = 0u64;
        let mut seen = HashSet::new();
        for entry in entries {
            if entry.is_symlink {
                return Err(format!(
                    "Archive contains a symlink entry: {}",
                    entry.relative_path
                ));
            }
            let relative = if entry.is_directory {
                entry.relative_path.trim_end_matches(|character| character == '/' || character == '\\')
            } else {
                entry.relative_path.as_str()
            };
            if relative.is_empty() {
                return Err(format!(
                    "Archive contains an empty entry path: {}",
                    entry.relative_path
                ));
            }
            let normalized = normalize_portable_relative_path(relative).map_err(|reason| {
                format!(
                    "Unsafe archive entry path '{}': {reason}",
                    entry.relative_path
                )
            })?;
            if normalized.len() > limits.max_path_bytes {
                return Err(format!(
                    "Archive entry path exceeds the {} byte limit: {}",
                    limits.max_path_bytes, entry.relative_path
                ));
            }
            if !seen.insert(portable_path_key(&normalized)) {
                return Err(format!("Archive contains duplicate entry path: {normalized}"));
            }
            if !entry.is_directory {
                total_bytes = total_bytes
                    .checked_add(entry.uncompressed_size)
                    .ok_or_else(|| "Archive uncompressed size overflow.".to_string())?;
                if total_bytes > limits.max_uncompressed_bytes {
                    return Err(format!(
                        "Archive uncompressed data exceeds the {} byte limit.",
                        limits.max_uncompressed_bytes
                    ));
                }
            }
        }
        Ok(())
    }
}

fn replace_file(temp: &Path, destination: &Path) -> Result<(), String> {
    match fs::rename(temp, destination) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if !destination.exists() {
                return Err(format!("Cannot finalize atomic write: {first_error}"));
            }
            let backup = net::temp_path_near(destination, "bak");
            fs::rename(destination, &backup)
                .map_err(|error| format!("Cannot move old file aside: {error}"))?;
            match fs::rename(temp, destination) {
                Ok(()) => {
                    let _ = fs::remove_file(&backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, destination);
                    Err(format!("Cannot finalize atomic write: {error}"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_port_rejects_traversal_and_accepts_safe_relative_paths() {
        let workspace = SafeWorkspace;
        assert_eq!(
            workspace
                .safe_join(Path::new("root"), "nested/file.bin")
                .unwrap(),
            Path::new("root").join("nested").join("file.bin")
        );
        assert!(workspace.safe_join(Path::new("root"), "../escape").is_err());
        assert!(workspace.safe_join(Path::new("root"), "C:/escape").is_err());
    }

    #[test]
    fn archive_policy_rejects_unsafe_duplicate_and_oversized_entries() {
        let workspace = SafeWorkspace;
        let limits = ArchiveLimits {
            max_entries: 3,
            max_path_bytes: 32,
            max_uncompressed_bytes: 10,
        };
        assert!(workspace
            .validate_archive_entries(
                &[
                    ArchiveEntry::new("nested/file.bin", 4, false),
                    ArchiveEntry::new("nested/other.bin", 5, false),
                ],
                &limits,
            )
            .is_ok());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("../escape", 1, false)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(
                &[
                    ArchiveEntry::new("same.bin", 1, false),
                    ArchiveEntry::new("SAME.bin", 1, false),
                ],
                &limits,
            )
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("large.bin", 11, false)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(
                &[
                    ArchiveEntry::new("one", 1, false),
                    ArchiveEntry::new("two", 1, false),
                    ArchiveEntry::new("three", 1, false),
                    ArchiveEntry::new("four", 1, false),
                ],
                &limits,
            )
            .is_err());
    }

    #[test]
    fn windows_portable_paths_reject_reserved_and_invalid_components() {
        let workspace = SafeWorkspace;
        for path in [
            "CON",
            "con.txt",
            "Aux.tar.gz",
            "COM1",
            "lpt9.txt",
            "CONIN$.txt",
            "COM1 .txt",
            "name.",
            "name ",
            "bad?.txt",
            "bad*.txt",
            "bad\"name",
            "bad<name>",
            "bad|name",
        ] {
            assert!(
                workspace.safe_join(Path::new("root"), path).is_err(),
                "path should be rejected: {path}"
            );
        }
        assert!(workspace
            .safe_join(Path::new("root"), "bad\u{0}name")
            .is_err());
        assert!(workspace
            .safe_join(Path::new("root"), "COM10.txt")
            .is_ok());
        assert!(workspace
            .safe_join(Path::new("root"), "my file.txt")
            .is_ok());
        assert!(workspace.safe_join(Path::new("root"), "caf\u{e9}.txt").is_ok());
    }

    #[test]
    fn archive_policy_accepts_directory_entries_and_rejects_symlinks() {
        let workspace = SafeWorkspace;
        let limits = ArchiveLimits {
            max_entries: 4,
            max_path_bytes: 64,
            max_uncompressed_bytes: 10,
        };
        assert!(workspace
            .validate_archive_entries(
                &[
                    ArchiveEntry::new("folder/", 0, true),
                    ArchiveEntry::new("nested\\sub\\", 0, true),
                    ArchiveEntry::new("folder/file.bin", 4, false),
                ],
                &limits,
            )
            .is_ok());
        assert!(workspace
            .validate_archive_entries(
                &[
                    ArchiveEntry::new("folder/", 0, true),
                    ArchiveEntry::new("FOLDER/", 0, true),
                ],
                &limits,
            )
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("", 0, true)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("///", 0, true)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("\\\\", 0, true)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("folder/", 1, false)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("dir/CON.txt", 1, false)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("dir/name.", 1, false)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("dir/bad*.txt", 1, false)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::new("name", 1, false), ArchiveEntry::new("name.", 1, false)], &limits)
            .is_err());
        assert!(workspace
            .validate_archive_entries(&[ArchiveEntry::symlink("link", 0)], &limits)
            .is_err());
    }

    #[test]
    fn archive_duplicate_detection_uses_unicode_lowercase() {
        let workspace = SafeWorkspace;
        let limits = ArchiveLimits {
            max_entries: 4,
            max_path_bytes: 64,
            max_uncompressed_bytes: 10,
        };
        let error = workspace
            .validate_archive_entries(
                &[
                    ArchiveEntry::new("\u{00C4}.txt", 1, false),
                    ArchiveEntry::new("\u{00E4}.txt", 1, false),
                ],
                &limits,
            )
            .unwrap_err();
        assert!(error.contains("duplicate entry path"));
    }

    #[test]
    fn atomic_write_replaces_the_destination() {
        let root = std::env::temp_dir().join(format!("flint-core-workspace-{}", net::nonce()));
        let destination = root.join("nested").join("state.bin");
        let workspace = SafeWorkspace;
        workspace.write_atomic(&destination, b"first").unwrap();
        workspace.write_atomic(&destination, b"second").unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"second".to_vec());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn path_port_is_object_safe_and_send_sync() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn PathPort>();
        assert_send_sync::<SafeWorkspace>();
    }
}
