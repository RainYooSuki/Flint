use crate::models::JavaInfo;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaDetails {
    pub version: String,
    pub major: u32,
    pub architecture: String,
}

pub fn java_executable() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

pub fn inspect_java(path: &Path) -> Result<JavaDetails, String> {
    let mut command = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = command
        .arg("-XshowSettings:properties")
        .arg("-version")
        .output()
        .map_err(|error| format!("Cannot run Java executable ({}): {error}", path.display()))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let (version, major) = parse_java_version(&text).ok_or_else(|| {
        format!(
            "Could not parse the Java version reported by {}",
            path.display()
        )
    })?;
    let architecture = parse_java_architecture(&text);
    Ok(JavaDetails {
        version,
        major,
        architecture,
    })
}

pub fn resolve_java_path(value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(PathBuf::from(java_executable()));
    }

    let path = PathBuf::from(trimmed);
    if path.is_file() {
        return canonicalize_java_file(path);
    }
    if path.is_dir() {
        for relative in [
            PathBuf::from("bin").join(java_executable()),
            PathBuf::from("Contents")
                .join("Home")
                .join("bin")
                .join(java_executable()),
            PathBuf::from("jre").join("bin").join(java_executable()),
        ] {
            let candidate = path.join(relative);
            if candidate.is_file() {
                return canonicalize_java_file(candidate);
            }
        }
    }

    Err(format!("Java executable not found: {trimmed}"))
}

fn canonicalize_java_file(path: PathBuf) -> Result<PathBuf, String> {
    fs::canonicalize(&path).map_err(|error| {
        format!(
            "Cannot resolve Java executable ({}): {error}",
            path.display()
        )
    })
}

pub fn scan_java() -> Vec<JavaInfo> {
    let mut candidates = Vec::new();

    if let Some(java_home) = env::var_os("JAVA_HOME") {
        collect_home_candidates(Path::new(&java_home), &mut candidates);
    }

    if let Some(path_value) = env::var_os("PATH") {
        for directory in env::split_paths(&path_value) {
            candidates.push(directory.join(java_executable()));
        }
    }

    let mut roots = Vec::new();
    if cfg!(windows) {
        for variable in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(base) = env::var_os(variable) {
                let base = PathBuf::from(base);
                for vendor in [
                    "Java",
                    "Eclipse Adoptium",
                    "Microsoft",
                    "Zulu",
                    "BellSoft",
                    "Amazon Corretto",
                    "AdoptOpenJDK",
                    "Semeru",
                    "Temurin",
                ] {
                    roots.push(base.join(vendor));
                }
            }
        }
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            let programs = PathBuf::from(local_app_data).join("Programs");
            for vendor in [
                "Eclipse Adoptium",
                "Microsoft",
                "Zulu",
                "BellSoft",
                "Amazon Corretto",
                "Java",
            ] {
                roots.push(programs.join(vendor));
            }
        }
        roots.push(PathBuf::from(r"C:\Program Files\Java"));
    } else if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        if let Some(home) = dirs::home_dir() {
            roots.push(
                home.join("Library")
                    .join("Java")
                    .join("JavaVirtualMachines"),
            );
        }
    } else {
        roots.push(PathBuf::from("/usr/lib/jvm"));
        roots.push(PathBuf::from("/usr/lib64/jvm"));
        roots.push(PathBuf::from("/usr/java"));
        roots.push(PathBuf::from("/opt/java"));
        roots.push(PathBuf::from("/opt/jdks"));
        roots.push(PathBuf::from("/usr/local/opt"));
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".sdkman").join("candidates").join("java"));
        }
    }

    for root in roots {
        scan_java_root(&root, 2, &mut candidates);
    }

    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for candidate in candidates {
        let canonical = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());
        if !canonical.is_file() {
            continue;
        }
        let key = if cfg!(windows) {
            canonical.to_string_lossy().to_ascii_lowercase()
        } else {
            canonical.to_string_lossy().to_string()
        };
        if !seen.insert(key) {
            continue;
        }
        let details = match inspect_java(&canonical) {
            Ok(value) => value,
            Err(_) => continue,
        };
        result.push(JavaInfo {
            path: canonical.display().to_string(),
            version: details.version,
            major: details.major,
            architecture: details.architecture,
        });
    }

    result.sort_by(|left, right| {
        right
            .major
            .cmp(&left.major)
            .then_with(|| left.path.cmp(&right.path))
    });
    result
}

fn scan_java_root(root: &Path, depth: usize, candidates: &mut Vec<PathBuf>) {
    collect_home_candidates(root, candidates);
    if depth == 0 {
        return;
    }
    let entries = match fs::read_dir(root) {
        Ok(value) => value,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let is_dir = entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        if is_dir {
            scan_java_root(&entry.path(), depth - 1, candidates);
        }
    }
}

fn collect_home_candidates(home: &Path, candidates: &mut Vec<PathBuf>) {
    candidates.push(home.join("bin").join(java_executable()));
    candidates.push(home.join("jre").join("bin").join(java_executable()));
    candidates.push(
        home.join("Contents")
            .join("Home")
            .join("bin")
            .join(java_executable()),
    );
}

pub fn parse_java_version(text: &str) -> Option<(String, u32)> {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("java.version") || lower.contains("java.runtime.version") {
            if let Some((_, value)) = line.split_once('=') {
                if let Some(parsed) = normalize_java_version(value) {
                    return Some(parsed);
                }
            }
        }
    }

    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(position) = lower.find("version") {
            let after = &line[position + "version".len()..];
            if let Some(start) = after.find('"') {
                if let Some(end) = after[start + 1..].find('"') {
                    if let Some(parsed) = normalize_java_version(&after[start + 1..start + 1 + end])
                    {
                        return Some(parsed);
                    }
                }
            }
            if let Some(parsed) = first_version_token(after) {
                return Some(parsed);
            }
        }
    }

    for line in text.lines() {
        if let Some(parsed) = first_version_token(line) {
            return Some(parsed);
        }
    }
    None
}

fn normalize_java_version(value: &str) -> Option<(String, u32)> {
    let (token, major) = first_version_token(value)?;
    Some((token, major))
}

fn first_version_token(value: &str) -> Option<(String, u32)> {
    for raw in value.split_whitespace() {
        let token = raw
            .trim_matches(|character: char| {
                character == '"' || character == '\'' || character == ',' || character == ';'
            })
            .trim();
        if token.is_empty() || !token.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        if !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
        {
            continue;
        }
        if let Some(major) = java_major_from_version(token) {
            return Some((token.to_string(), major));
        }
    }
    None
}

fn java_major_from_version(version: &str) -> Option<u32> {
    let first_end = version
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(version.len());
    if first_end == 0 {
        return None;
    }
    let first: u32 = version[..first_end].parse().ok()?;
    if first == 1 {
        let rest = version[first_end..].trim_start_matches('.');
        let second_end = rest
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(rest.len());
        if second_end == 0 {
            return None;
        }
        rest[..second_end].parse().ok()
    } else if first == 0 {
        None
    } else {
        Some(first)
    }
}

pub fn parse_java_architecture(text: &str) -> String {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("os.arch") {
            if let Some((_, value)) = line.split_once('=') {
                let normalized = normalize_architecture(value.trim());
                if !normalized.is_empty() {
                    return normalized;
                }
            }
        }
    }
    normalize_architecture(env::consts::ARCH)
}

fn normalize_architecture(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" | "x64" => "x86_64".to_string(),
        "x86" | "i386" | "i486" | "i586" | "i686" | "32" => "x86".to_string(),
        "aarch64" | "arm64" => "aarch64".to_string(),
        "arm" | "armv7l" | "armv7" => "arm".to_string(),
        "ppc64le" | "ppc64" => "ppc64".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_java_version_and_properties() {
        let output = "openjdk version \"21.0.2\" 2024-01-16\nProperty settings:\n    java.version = 21.0.2\n    os.arch = amd64\n";
        assert_eq!(parse_java_version(output), Some(("21.0.2".to_string(), 21)));
        assert_eq!(parse_java_architecture(output), "x86_64");
    }

    #[test]
    fn parses_legacy_java_8_version() {
        let output = "java version \"1.8.0_402\"\nJava(TM) SE Runtime Environment";
        assert_eq!(
            parse_java_version(output),
            Some(("1.8.0_402".to_string(), 8))
        );
    }

    #[test]
    fn parses_unquoted_openjdk_versions() {
        assert_eq!(
            parse_java_version("openjdk 17.0.10 2024-01-16\nOpenJDK Runtime Environment"),
            Some(("17.0.10".to_string(), 17))
        );
    }

    #[test]
    fn architecture_normalization_handles_common_aliases() {
        assert_eq!(normalize_architecture("AMD64"), "x86_64");
        assert_eq!(normalize_architecture("aarch64"), "aarch64");
        assert_eq!(normalize_architecture("i386"), "x86");
    }

    #[test]
    fn rejects_non_version_text() {
        assert_eq!(parse_java_version("java version unknown"), None);
    }
}
