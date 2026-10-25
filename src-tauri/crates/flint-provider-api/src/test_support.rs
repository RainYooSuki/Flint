//! Reusable in-memory ports for provider API tests. This module is not part of
//! the default public API and never performs network, process, or filesystem I/O.

use flint_core::minecraft::{parse_json, JsonValue};
use flint_core::ports::{
    DownloadRequest, JavaRunRequest, JavaRunner, NetworkPort, NetworkRequest,
    PathPort, ProcessOutput, SafeWorkspace,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Debug, Default)]
pub struct FakeNetworkPort {
    responses: Mutex<VecDeque<Result<Vec<u8>, String>>>,
    pub requests: Mutex<Vec<NetworkRequest>>,
    pub downloads: Mutex<Vec<DownloadRequest>>,
}

impl FakeNetworkPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_response(&self, bytes: impl Into<Vec<u8>>) {
        self.responses
            .lock()
            .expect("fake network response lock")
            .push_back(Ok(bytes.into()));
    }

    pub fn push_error(&self, error: impl Into<String>) {
        self.responses
            .lock()
            .expect("fake network response lock")
            .push_back(Err(error.into()));
    }

    pub fn requests_len(&self) -> usize {
        self.requests.lock().expect("fake network request lock").len()
    }

    fn next_response(&self) -> Result<Vec<u8>, String> {
        self.responses
            .lock()
            .expect("fake network response lock")
            .pop_front()
            .unwrap_or_else(|| Err("fake network has no queued response".to_string()))
    }
}

impl NetworkPort for FakeNetworkPort {
    fn read_json(&self, request: &NetworkRequest) -> Result<JsonValue, String> {
        self.requests
            .lock()
            .expect("fake network request lock")
            .push(request.clone());
        let bytes = self.next_response()?;
        parse_json(&bytes)
    }

    fn read_text(&self, request: &NetworkRequest) -> Result<String, String> {
        self.requests
            .lock()
            .expect("fake network request lock")
            .push(request.clone());
        let bytes = self.next_response()?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    fn read_bytes(&self, request: &NetworkRequest) -> Result<Vec<u8>, String> {
        self.requests
            .lock()
            .expect("fake network request lock")
            .push(request.clone());
        self.next_response()
    }

    fn download_atomic(&self, request: &DownloadRequest) -> Result<u64, String> {
        self.downloads
            .lock()
            .expect("fake network download lock")
            .push(request.clone());
        Ok(0)
    }
}

#[derive(Debug)]
pub struct FakeJavaRunner {
    pub requests: Mutex<Vec<JavaRunRequest>>,
    pub output: ProcessOutput,
}

impl FakeJavaRunner {
    pub fn new() -> Self {
        Self::with_output(ProcessOutput {
            status_code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
            timed_out: false,
        })
    }

    pub fn with_output(output: ProcessOutput) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            output,
        }
    }

    pub fn requests_len(&self) -> usize {
        self.requests.lock().expect("fake Java request lock").len()
    }
}

impl Default for FakeJavaRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaRunner for FakeJavaRunner {
    fn run_java(&self, request: JavaRunRequest) -> Result<ProcessOutput, String> {
        self.requests
            .lock()
            .expect("fake Java request lock")
            .push(request);
        Ok(self.output.clone())
    }
}

#[derive(Debug, Default)]
pub struct FakePathPort {
    files: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
    dirs: Mutex<BTreeSet<PathBuf>>,
    symlinks: Mutex<BTreeSet<PathBuf>>,
}

impl FakePathPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_directory(&self, path: impl Into<PathBuf>) {
        let path = path.into();
        self.insert_with_parents(&path);
    }

    pub fn insert_file(&self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) {
        let path = path.into();
        self.insert_file_with_parents(&path, bytes.into());
    }

    pub fn insert_symlink(&self, path: impl Into<PathBuf>) {
        let path = path.into();
        if let Some(parent) = path.parent() {
            self.insert_with_parents(parent);
        }
        self.symlinks
            .lock()
            .expect("fake workspace symlink lock")
            .insert(path);
    }

    pub fn file_bytes(&self, path: &Path) -> Option<Vec<u8>> {
        self.files
            .lock()
            .expect("fake workspace file lock")
            .get(path)
            .cloned()
    }

    fn insert_with_parents(&self, path: &Path) {
        let mut current = Some(path);
        while let Some(path) = current {
            if !path.as_os_str().is_empty() {
                self.dirs
                    .lock()
                    .expect("fake workspace directory lock")
                    .insert(path.to_path_buf());
            }
            current = path.parent();
        }
    }

    fn insert_file_with_parents(&self, path: &Path, bytes: Vec<u8>) {
        if let Some(parent) = path.parent() {
            self.insert_with_parents(parent);
        }
        self.files
            .lock()
            .expect("fake workspace file lock")
            .insert(path.to_path_buf(), bytes);
    }
}

impl PathPort for FakePathPort {
    fn safe_join(&self, base: &Path, relative: &str) -> Result<PathBuf, String> {
        flint_core::paths::safe_join(base, relative)
            .ok_or_else(|| format!("Unsafe relative path: {relative}"))
    }

    fn ensure_directory(&self, path: &Path) -> Result<(), String> {
        self.insert_with_parents(path);
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, String> {
        self.file_bytes(path)
            .ok_or_else(|| format!("Missing fake file: {}", path.display()))
    }

    fn read_file_prefix(&self, path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
        let bytes = self.read_file(path)?;
        Ok(bytes.into_iter().take(max_bytes).collect())
    }

    fn write_file(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        self.insert_file_with_parents(path, bytes.to_vec());
        Ok(())
    }

    fn write_atomic(&self, destination: &Path, bytes: &[u8]) -> Result<(), String> {
        self.write_file(destination, bytes)
    }

    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, String> {
        let mut children = BTreeSet::new();
        {
            let dirs = self
                .dirs
                .lock()
                .expect("fake workspace directory lock");
            for candidate in dirs.iter() {
                if candidate.parent() == Some(path) {
                    children.insert(candidate.clone());
                }
            }
        }
        {
            let files = self
                .files
                .lock()
                .expect("fake workspace file lock");
            for candidate in files.keys() {
                if candidate.parent() == Some(path) {
                    children.insert(candidate.clone());
                }
            }
        }
        {
            let symlinks = self
                .symlinks
                .lock()
                .expect("fake workspace symlink lock");
            for candidate in symlinks.iter() {
                if candidate.parent() == Some(path) {
                    children.insert(candidate.clone());
                }
            }
        }
        Ok(children.into_iter().collect())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), String> {
        let bytes = self.read_file(from)?;
        self.remove_file(from)?;
        self.write_file(to, &bytes)
    }

    fn remove_file(&self, path: &Path) -> Result<(), String> {
        self.files
            .lock()
            .expect("fake workspace file lock")
            .remove(path);
        self.symlinks
            .lock()
            .expect("fake workspace symlink lock")
            .remove(path);
        Ok(())
    }

    fn remove_directory(&self, path: &Path) -> Result<(), String> {
        self.files
            .lock()
            .expect("fake workspace file lock")
            .retain(|candidate, _| !candidate.starts_with(path));
        self.dirs
            .lock()
            .expect("fake workspace directory lock")
            .retain(|candidate| !candidate.starts_with(path));
        self.symlinks
            .lock()
            .expect("fake workspace symlink lock")
            .retain(|candidate| !candidate.starts_with(path));
        Ok(())
    }

    fn is_regular_file(&self, path: &Path) -> bool {
        self.files
            .lock()
            .expect("fake workspace file lock")
            .contains_key(path)
    }

    fn is_symlink(&self, path: &Path) -> bool {
        self.symlinks
            .lock()
            .expect("fake workspace symlink lock")
            .contains(path)
    }

    fn is_directory(&self, path: &Path) -> bool {
        self.dirs
            .lock()
            .expect("fake workspace directory lock")
            .contains(path)
    }

    fn modified(&self, _path: &Path) -> Option<std::time::SystemTime> {
        None
    }

    fn validate_archive_entries(
        &self,
        entries: &[flint_core::ports::ArchiveEntry],
        limits: &flint_core::ports::ArchiveLimits,
    ) -> Result<(), String> {
        PathPort::validate_archive_entries(&SafeWorkspace, entries, limits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_core::ports::{NetworkPort, PathPort};
    use std::time::Duration;

    #[test]
    fn fake_ports_are_in_memory_and_object_safe() {
        let network = FakeNetworkPort::new();
        network.push_response(br#"{"ok":true}"#.to_vec());
        let request = NetworkRequest::new(
            "https://example.com/api",
            flint_core::ports::ServiceLabel::new("Example").unwrap(),
            1024,
            Duration::from_secs(75),
        );
        let value = NetworkPort::read_json(&network, &request).unwrap();
        assert_eq!(value.get("ok").and_then(|value| value.as_bool()), Some(true));
        let path = FakePathPort::new();
        path.insert_file("game/launcher_profiles.json", b"{}".to_vec());
        assert!(path.is_regular_file(Path::new("game/launcher_profiles.json")));
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<FakeNetworkPort>();
        assert_send_sync::<FakeJavaRunner>();
        assert_send_sync::<FakePathPort>();
    }
}
