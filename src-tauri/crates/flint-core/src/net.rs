use reqwest::blocking::{Client, Response};
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const DOWNLOAD_ATTEMPTS: usize = 3;

pub fn http_client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!(
            "Flint/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/Flint-Launcher/Flint)"
        ))
        .connect_timeout(Duration::from_secs(20))
        .timeout(timeout)
        .https_only(true)
        .build()
        .map_err(|error| format!("Cannot initialize HTTP client: {error}"))
}

pub fn validate_https_url(url: &str) -> Result<(), String> {
    if url.starts_with("https://") {
        Ok(())
    } else {
        Err("Refusing to use a non-HTTPS URL.".to_string())
    }
}

pub fn read_body_limited(mut response: Response, max_bytes: u64) -> Result<Vec<u8>, String> {
    if let Some(length) = response.content_length() {
        if length > max_bytes {
            return Err(format!(
                "HTTP response is too large ({length} bytes, limit {max_bytes})."
            ));
        }
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(|error| format!("Cannot read HTTP response: {error}"))?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > max_bytes {
            return Err(format!("HTTP response exceeds the {max_bytes} byte limit."));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Ok(bytes)
}

pub fn download_atomic(
    client: &Client,
    url: &str,
    destination: &Path,
    expected_sha1: Option<&str>,
    max_bytes: u64,
    label: &str,
) -> Result<u64, String> {
    validate_https_url(url)?;
    if let Some(expected) = expected_sha1 {
        if !crate::paths::is_sha1_hex(expected) {
            return Err(format!("Invalid SHA-1 supplied for {label}."));
        }
        if destination.is_file()
            && crate::paths::sha1_file(destination)
                .map(|actual| actual.eq_ignore_ascii_case(expected))
                .unwrap_or(false)
        {
            return Ok(0);
        }
    } else if destination
        .metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
    {
        return Ok(0);
    }

    let parent = destination.parent().ok_or_else(|| {
        format!(
            "Download destination has no parent: {}",
            destination.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Cannot create download directory ({}): {error}",
            parent.display()
        )
    })?;

    let mut last_error = String::new();
    for attempt in 1..=DOWNLOAD_ATTEMPTS {
        match download_once(client, url, destination, expected_sha1, max_bytes, label) {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                last_error = error;
                if attempt < DOWNLOAD_ATTEMPTS {
                    thread::sleep(Duration::from_millis(400 * attempt as u64));
                }
            }
        }
    }
    Err(format!(
        "Download failed for {label} after {DOWNLOAD_ATTEMPTS} attempts: {last_error}"
    ))
}

fn download_once(
    client: &Client,
    url: &str,
    destination: &Path,
    expected_sha1: Option<&str>,
    max_bytes: u64,
    label: &str,
) -> Result<u64, String> {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download.bin");
    let part =
        destination.with_file_name(format!(".{file_name}.{}.{}.part", process::id(), nonce()));

    let result = (|| -> Result<u64, String> {
        let mut response = client
            .get(url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("HTTP download failed: {error}"))?;
        if let Some(length) = response.content_length() {
            if length > max_bytes {
                return Err(format!(
                    "{label} is too large ({length} bytes, limit {max_bytes})."
                ));
            }
        }

        let mut file = fs::File::create(&part)
            .map_err(|error| format!("Cannot create partial download: {error}"))?;
        let mut hasher = Sha1::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = response
                .read(&mut buffer)
                .map_err(|error| format!("Cannot read download response: {error}"))?;
            if count == 0 {
                break;
            }
            bytes = bytes
                .checked_add(count as u64)
                .ok_or_else(|| "Download size overflow.".to_string())?;
            if bytes > max_bytes {
                return Err(format!(
                    "{label} exceeds the {max_bytes} byte download limit."
                ));
            }
            file.write_all(&buffer[..count])
                .map_err(|error| format!("Cannot write partial download: {error}"))?;
            hasher.update(&buffer[..count]);
        }
        file.flush()
            .map_err(|error| format!("Cannot flush partial download: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Cannot sync partial download: {error}"))?;
        drop(file);

        if bytes == 0 {
            return Err(format!("{label} downloaded as an empty file."));
        }
        if let Some(expected) = expected_sha1 {
            let actual = format!("{:x}", hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(format!(
                    "SHA-1 mismatch for {label}: expected {expected}, received {actual}."
                ));
            }
        }

        commit_part(&part, destination)?;
        Ok(bytes)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn commit_part(part: &Path, destination: &Path) -> Result<(), String> {
    match fs::rename(part, destination) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if !destination.exists() {
                return Err(format!("Cannot finalize download: {first_error}"));
            }
            let file_name = destination
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("download.bin");
            let backup = destination.with_file_name(format!(
                ".{file_name}.{}.{}.bak",
                process::id(),
                nonce()
            ));
            fs::rename(destination, &backup)
                .map_err(|error| format!("Cannot move old download aside: {error}"))?;
            match fs::rename(part, destination) {
                Ok(()) => {
                    let _ = fs::remove_file(&backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, destination);
                    Err(format!("Cannot finalize download: {error}"))
                }
            }
        }
    }
}

pub fn ensure_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Cannot create directory ({}): {error}", path.display()))
}

pub fn temp_path_near(destination: &Path, suffix: &str) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    destination.with_file_name(format!(
        ".{file_name}.{}.{}.{suffix}",
        process::id(),
        nonce()
    ))
}

pub fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_url_validation_rejects_other_schemes() {
        assert!(validate_https_url("https://example.com/file.jar").is_ok());
        assert!(validate_https_url("http://example.com/file.jar").is_err());
        assert!(validate_https_url("file:///tmp/file.jar").is_err());
    }

    #[test]
    fn temp_paths_stay_in_the_destination_directory() {
        let destination = Path::new("downloads").join("example.jar");
        let temp = temp_path_near(&destination, "part");
        assert_eq!(temp.parent(), destination.parent());
        assert_ne!(temp, destination);
    }
}
