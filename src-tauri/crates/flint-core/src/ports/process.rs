//! Bounded process and Java-runner port contracts.

use std::{
    path::PathBuf,
    time::Duration,
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

#[cfg(test)]
mod tests {
    use super::*;

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