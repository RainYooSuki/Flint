//! Bounded process and Java-runner port contracts.

use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputLimits {
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self {
            stdout_bytes: 64 * 1024,
            stderr_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRequest {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub environment: Vec<(String, String)>,
    pub timeout: Duration,
    pub output_limits: OutputLimits,
}

impl ProcessRequest {
    pub fn new(
        program: impl Into<PathBuf>,
        args: Vec<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            program: program.into(),
            args,
            current_dir: None,
            environment: Vec::new(),
            timeout,
            output_limits: OutputLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
}

impl ProcessOutput {
    pub fn succeeded(&self) -> bool {
        !self.timed_out && self.status_code == Some(0)
    }
}

/// Object-safe bounded process contract. Concrete execution may live in a
/// platform adapter; callers must not reach for `std::process` directly.
pub trait ProcessPort: Send + Sync {
    fn run(&self, request: ProcessRequest) -> Result<ProcessOutput, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRunRequest {
    pub java_executable: PathBuf,
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub program_args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub timeout: Duration,
    pub output_limits: OutputLimits,
}

impl JavaRunRequest {
    pub fn new(
        java_executable: impl Into<PathBuf>,
        main_class: impl Into<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            java_executable: java_executable.into(),
            jvm_args: Vec::new(),
            main_class: main_class.into(),
            program_args: Vec::new(),
            current_dir: None,
            timeout,
            output_limits: OutputLimits::default(),
        }
    }

    pub fn to_process_request(&self) -> ProcessRequest {
        let mut args = Vec::with_capacity(
            self.jvm_args.len() + 1 + self.program_args.len(),
        );
        args.extend(self.jvm_args.iter().cloned());
        args.push(self.main_class.clone());
        args.extend(self.program_args.iter().cloned());
        ProcessRequest {
            program: self.java_executable.clone(),
            args,
            current_dir: self.current_dir.clone(),
            environment: Vec::new(),
            timeout: self.timeout,
            output_limits: self.output_limits,
        }
    }
}

/// Object-safe Java execution contract with bounded output and timeout fields.
pub trait JavaRunner: Send + Sync {
    fn run_java(&self, request: JavaRunRequest) -> Result<ProcessOutput, String>;
}

/// Concrete bounded process adapter backed by `std::process::Command`.
///
/// Timeout policy: the direct child is killed and waited on, then capture
/// threads are detached after taking a bounded snapshot. This prevents a
/// descendant that inherited stdout/stderr from blocking the caller forever.
/// Reader threads may remain parked until the inherited pipes close; they do
/// not accumulate unbounded output because capture buffers are hard-limited.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommandProcessPort;

impl ProcessPort for CommandProcessPort {
    fn run(&self, request: ProcessRequest) -> Result<ProcessOutput, String> {
        let mut command = Command::new(&request.program);
        command
            .args(&request.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(current_dir) = &request.current_dir {
            command.current_dir(current_dir);
        }
        for (name, value) in &request.environment {
            command.env(name.as_str(), value.as_str());
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        let mut child = command.spawn().map_err(|error| {
            format!(
                "Cannot start process ({}): {error}",
                request.program.display()
            )
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Process stdout was not captured.".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Process stderr was not captured.".to_string())?;
        let limits = request.output_limits;
        let (mut stdout_thread, stdout_state) = spawn_capture(stdout, limits.stdout_bytes, "stdout");
        let (mut stderr_thread, stderr_state) = spawn_capture(stderr, limits.stderr_bytes, "stderr");
        let deadline = Instant::now() + request.timeout;
        let mut child_timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("Cannot monitor process output: {error}"));
                }
            }
            if Instant::now() >= deadline {
                child_timed_out = true;
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            thread::sleep(Duration::from_millis(25));
        };

        if child_timed_out {
            // The direct child is gone, but a descendant may still hold the
            // inherited stdout/stderr pipes. Take a bounded snapshot and drop
            // the reader handles instead of joining them forever.
            let stdout = capture_snapshot(&stdout_state);
            let stderr = capture_snapshot(&stderr_state);
            let _ = stdout_thread.take();
            let _ = stderr_thread.take();
            return Ok(ProcessOutput {
                status_code: status.and_then(|status| status.code()),
                stdout: stdout.bytes,
                stderr: stderr.bytes,
                timed_out: true,
            });
        }

        // A child can exit while a descendant keeps its pipes open. Joining
        // capture threads is bounded by the same operation deadline; if the
        // deadline expires, detach the readers and return the bounded snapshot.
        let stdout_wait = await_capture(&mut stdout_thread, deadline, "stdout")?;
        let stderr_wait = await_capture(&mut stderr_thread, deadline, "stderr")?;
        let stdout = capture_snapshot(&stdout_state);
        let stderr = capture_snapshot(&stderr_state);
        if matches!(stdout_wait, CaptureWait::Joined) {
            if let Some(error) = stdout.error {
                return Err(error);
            }
        }
        if matches!(stderr_wait, CaptureWait::Joined) {
            if let Some(error) = stderr.error {
                return Err(error);
            }
        }
        Ok(ProcessOutput {
            status_code: status.and_then(|status| status.code()),
            stdout: stdout.bytes,
            stderr: stderr.bytes,
            timed_out: matches!(stdout_wait, CaptureWait::Detached)
                || matches!(stderr_wait, CaptureWait::Detached),
        })
    }
}

#[derive(Debug, Default)]
struct CaptureState {
    bytes: Vec<u8>,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptureSnapshot {
    bytes: Vec<u8>,
    error: Option<String>,
}

impl CaptureState {
    fn snapshot(&self) -> CaptureSnapshot {
        CaptureSnapshot {
            bytes: self.bytes.clone(),
            error: self.error.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureWait {
    Joined,
    Detached,
}

fn lock_capture(state: &Arc<Mutex<CaptureState>>) -> std::sync::MutexGuard<'_, CaptureState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn capture_snapshot(state: &Arc<Mutex<CaptureState>>) -> CaptureSnapshot {
    lock_capture(state).snapshot()
}

fn spawn_capture(
    mut reader: impl Read + Send + 'static,
    max_bytes: usize,
    stream: &'static str,
) -> (thread::JoinHandle<()>, Arc<Mutex<CaptureState>>) {
    let state = Arc::new(Mutex::new(CaptureState::default()));
    let thread_state = Arc::clone(&state);
    let handle = thread::spawn(move || {
        if let Err(error) = read_bounded_into(&mut reader, max_bytes, &thread_state) {
            lock_capture(&thread_state).error = Some(format!("Cannot read process {stream}: {error}"));
        }
    });
    (handle, state)
}

fn read_bounded_into(
    reader: &mut impl Read,
    max_bytes: usize,
    state: &Arc<Mutex<CaptureState>>,
) -> Result<(), String> {
    let mut buffer = [0u8; 8 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("Cannot read process output: {error}"))?;
        if count == 0 {
            break;
        }
        let mut capture = lock_capture(state);
        let remaining = max_bytes.saturating_sub(capture.bytes.len());
        if remaining > 0 {
            capture
                .bytes
                .extend_from_slice(&buffer[..count.min(remaining)]);
        }
    }
    Ok(())
}

fn await_capture(
    handle: &mut Option<thread::JoinHandle<()>>,
    deadline: Instant,
    stream: &str,
) -> Result<CaptureWait, String> {
    loop {
        let finished = match handle.as_ref() {
            Some(handle) => handle.is_finished(),
            None => true,
        };
        if finished {
            if let Some(handle) = handle.take() {
                handle
                    .join()
                    .map_err(|_| format!("Process {stream} capture thread panicked."))?;
            }
            return Ok(CaptureWait::Joined);
        }
        let now = Instant::now();
        if now >= deadline {
            // Dropping a JoinHandle detaches the reader. A descendant that
            // keeps the inherited pipe open must not hold up this call.
            let _ = handle.take();
            return Ok(CaptureWait::Detached);
        }
        let remaining = deadline.saturating_duration_since(now);
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

/// JavaRunner adapter over the object-safe ProcessPort.
pub struct ProcessJavaRunner<'a> {
    process: &'a dyn ProcessPort,
}

impl<'a> ProcessJavaRunner<'a> {
    pub fn new(process: &'a dyn ProcessPort) -> Self {
        Self { process }
    }
}

impl JavaRunner for ProcessJavaRunner<'_> {
    fn run_java(&self, request: JavaRunRequest) -> Result<ProcessOutput, String> {
        self.process.run(request.to_process_request())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct FakeProcessPort {
        requests: Arc<Mutex<Vec<ProcessRequest>>>,
    }

    impl ProcessPort for FakeProcessPort {
        fn run(&self, request: ProcessRequest) -> Result<ProcessOutput, String> {
            self.requests.lock().unwrap().push(request);
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
                timed_out: false,
            })
        }
    }

    #[test]
    fn bounded_reader_captures_only_the_configured_limit() {
        use std::io::Cursor;
        let state = Arc::new(Mutex::new(CaptureState::default()));
        let mut reader = Cursor::new(vec![1u8, 2, 3, 4]);
        read_bounded_into(&mut reader, 2, &state).unwrap();
        assert_eq!(capture_snapshot(&state).bytes, vec![1, 2]);
        let state = Arc::new(Mutex::new(CaptureState::default()));
        let mut empty = Cursor::new(vec![1u8, 2, 3]);
        read_bounded_into(&mut empty, 0, &state).unwrap();
        assert!(capture_snapshot(&state).bytes.is_empty());
    }

    #[test]
    fn normal_completion_joins_capture_and_preserves_bytes() {
        use std::io::Cursor;
        let (handle, state) = spawn_capture(
            Cursor::new(b"line one\r\nline two\n".to_vec()),
            1024,
            "stdout",
        );
        let mut handle = Some(handle);
        let wait = await_capture(
            &mut handle,
            Instant::now() + Duration::from_secs(1),
            "stdout",
        )
        .expect("normal capture");
        assert_eq!(wait, CaptureWait::Joined);
        let snapshot = capture_snapshot(&state);
        assert_eq!(snapshot.bytes, b"line one\r\nline two\n".to_vec());
        assert!(snapshot.error.is_none());
    }

    #[test]
    fn capture_wait_detaches_blocked_reader_at_deadline() {
        use std::sync::mpsc;
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let state = Arc::new(Mutex::new(CaptureState::default()));
        let thread_state = Arc::clone(&state);
        let handle = thread::spawn(move || {
            lock_capture(&thread_state)
                .bytes
                .extend_from_slice(b"partial");
            let _ = started_tx.send(());
            let _ = release_rx.recv();
        });
        started_rx.recv().expect("capture worker started");
        let mut handle = Some(handle);
        let wait = await_capture(&mut handle, Instant::now(), "test").expect("bounded capture wait");
        assert_eq!(wait, CaptureWait::Detached);
        assert!(handle.is_none());
        assert_eq!(capture_snapshot(&state).bytes, b"partial".to_vec());
        let _ = release_tx.send(());
    }

    #[test]
    fn process_java_runner_delegates_through_process_port() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let process = FakeProcessPort {
            requests: Arc::clone(&requests),
        };
        let runner = ProcessJavaRunner::new(&process);
        let mut request = JavaRunRequest::new("java", "-jar", Duration::from_secs(600));
        request.program_args = vec!["installer.jar".to_string(), "--install".to_string()];
        let output = runner.run_java(request).expect("fake process output");
        assert!(output.succeeded());
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].program, PathBuf::from("java"));
        assert_eq!(
            requests[0].args,
            vec![
                "-jar".to_string(),
                "installer.jar".to_string(),
                "--install".to_string()
            ]
        );
    }

    #[test]
    fn java_request_builds_ordered_bounded_process_request() {
        let mut request = JavaRunRequest::new(
            "java",
            "example.Main",
            Duration::from_secs(30),
        );
        request.jvm_args = vec!["-Xmx1G".to_string()];
        request.program_args = vec!["--version".to_string()];
        request.output_limits.stdout_bytes = 1024;
        let process = request.to_process_request();
        assert_eq!(
            process.args,
            vec![
                "-Xmx1G".to_string(),
                "example.Main".to_string(),
                "--version".to_string(),
            ]
        );
        assert_eq!(process.timeout, Duration::from_secs(30));
        assert_eq!(process.output_limits.stdout_bytes, 1024);
    }

    #[test]
    fn process_and_java_traits_are_object_safe_and_send_sync() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn ProcessPort>();
        assert_send_sync::<dyn JavaRunner>();
    }
}
