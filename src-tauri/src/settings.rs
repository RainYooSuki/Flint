use crate::models::LauncherSettings;
use crate::paths::settings_path;
use std::{
    fs,
    io::{self, Write},
    path::Path,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn validate_username(username: &str) -> Result<(), String> {
    if !(3..=16).contains(&username.len()) {
        return Err("Username must be between 3 and 16 characters.".to_string());
    }
    if !username
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err("Username may contain only letters, numbers and underscores.".to_string());
    }
    Ok(())
}

pub fn validate_settings(settings: &LauncherSettings) -> Result<(), String> {
    validate_username(&settings.username)?;

    if settings.min_memory_mb < 128 {
        return Err("Minimum memory must be at least 128 MB.".to_string());
    }
    if settings.max_memory_mb > 131_072 {
        return Err("Maximum memory must not exceed 131072 MB.".to_string());
    }
    if settings.min_memory_mb > settings.max_memory_mb {
        return Err("Minimum memory cannot exceed maximum memory.".to_string());
    }
    if !(320..=16_384).contains(&settings.width) {
        return Err("Window width must be between 320 and 16384.".to_string());
    }
    if !(240..=16_384).contains(&settings.height) {
        return Err("Window height must be between 240 and 16384.".to_string());
    }

    Ok(())
}

pub fn load_settings() -> Result<LauncherSettings, String> {
    load_settings_from_path(&settings_path())
}

fn load_settings_from_path(path: &Path) -> Result<LauncherSettings, String> {
    let text = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let defaults = LauncherSettings::default();
            save_settings_to_path(path, &defaults)?;
            return Ok(defaults);
        }
        Err(error) => {
            return Err(format!(
                "Cannot read settings file ({}): {error}",
                path.display()
            ));
        }
    };

    let settings: LauncherSettings = serde_json::from_str(&text)
        .map_err(|error| format!("Settings file is invalid JSON: {error}"))?;
    validate_settings(&settings)?;
    Ok(settings)
}

pub fn save_settings(settings: LauncherSettings) -> Result<LauncherSettings, String> {
    validate_settings(&settings)?;
    save_settings_to_path(&settings_path(), &settings)?;
    Ok(settings)
}

fn save_settings_to_path(path: &Path, settings: &LauncherSettings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Settings path has no parent directory.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Cannot create settings directory ({}): {error}",
            parent.display()
        )
    })?;

    let json = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Cannot serialize settings: {error}"))?;
    atomic_write(path, &json)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("settings.json");
    let temp = path.with_file_name(format!(".{file_name}.{}.{}.tmp", process::id(), nonce()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|error| format!("Cannot create temporary settings file: {error}"))?;
    let write_result = (|| -> Result<(), String> {
        file.write_all(bytes)
            .map_err(|error| format!("Cannot write temporary settings file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Cannot flush temporary settings file: {error}"))?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

    replace_file(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("Cannot replace settings file ({}): {error}", path.display())
    })
}

fn replace_file(temp: &Path, destination: &Path) -> io::Result<()> {
    match fs::rename(temp, destination) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            #[cfg(windows)]
            {
                if destination.exists() {
                    let backup = destination.with_file_name(format!(
                        ".{}.{}.{}.bak",
                        destination
                            .file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("settings.json"),
                        process::id(),
                        nonce()
                    ));
                    fs::rename(destination, &backup)?;
                    match fs::rename(temp, destination) {
                        Ok(()) => {
                            let _ = fs::remove_file(&backup);
                            Ok(())
                        }
                        Err(error) => {
                            let _ = fs::rename(&backup, destination);
                            Err(error)
                        }
                    }
                } else {
                    Err(first_error)
                }
            }
            #[cfg(not(windows))]
            {
                Err(first_error)
            }
        }
    }
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_validation_matches_contract() {
        assert!(validate_username("Alex").is_ok());
        assert!(validate_username("Player_123").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("abcdefghijklmnopq").is_err());
        assert!(validate_username("bad-name").is_err());
        assert!(validate_username("bad name").is_err());
        assert!(validate_username("鐜╁鍚嶅瓧").is_err());
    }

    #[test]
    fn settings_validation_checks_memory_and_window_ranges() {
        let mut settings = LauncherSettings::default();
        assert!(validate_settings(&settings).is_ok());

        settings.min_memory_mb = 4096;
        settings.max_memory_mb = 1024;
        assert!(validate_settings(&settings).is_err());

        settings.min_memory_mb = 1024;
        settings.max_memory_mb = 4096;
        settings.width = 100;
        assert!(validate_settings(&settings).is_err());
    }

    #[test]
    fn settings_serialize_with_camel_case_keys() {
        let value = serde_json::to_value(LauncherSettings::default()).expect("settings serialize");
        assert!(value.get("gameDir").is_some());
        assert!(value.get("javaPath").is_some());
        assert!(value.get("minMemoryMb").is_some());
        assert!(value.get("maxMemoryMb").is_some());
        assert!(value.get("jvmArgs").is_some());
        assert!(value.get("gameArgs").is_some());
        assert!(value.get("game_dir").is_none());
    }

    #[test]
    fn missing_settings_fields_use_defaults() {
        let parsed: LauncherSettings =
            serde_json::from_str(r#"{"username":"Alex"}"#).expect("settings deserialize");
        assert_eq!(parsed.username, "Alex");
        assert_eq!(parsed.min_memory_mb, 1024);
        assert_eq!(parsed.max_memory_mb, 4096);
    }
}
