//! Progress, logging, deadline, and cancellation contracts.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressUpdate {
    pub completed: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}

impl ProgressUpdate {
    pub fn new(completed: u64, total: Option<u64>, message: Option<String>) -> Self {
        Self {
            completed,
            total,
            message,
        }
    }
}

/// Object-safe progress sink for provider and feature work.
pub trait ProgressSink: Send + Sync {
    fn report(&self, update: ProgressUpdate);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn report(&self, _update: ProgressUpdate) {}
}

/// Object-safe logging contract. Messages must already be safe for display.
pub trait LoggerPort: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);

    fn enabled(&self, _level: LogLevel) -> bool {
        true
    }

    fn debug(&self, message: &str) {
        if self.enabled(LogLevel::Debug) {
            self.log(LogLevel::Debug, message);
        }
    }

    fn info(&self, message: &str) {
        if self.enabled(LogLevel::Info) {
            self.log(LogLevel::Info, message);
        }
    }

    fn warn(&self, message: &str) {
        if self.enabled(LogLevel::Warn) {
            self.log(LogLevel::Warn, message);
        }
    }

    fn error(&self, message: &str) {
        if self.enabled(LogLevel::Error) {
            self.log(LogLevel::Error, message);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopLogger;

impl LoggerPort for NoopLogger {
    fn log(&self, _level: LogLevel, _message: &str) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    expires_at: Option<Instant>,
}

impl Deadline {
    /// A non-expiring deadline for operations whose individual requests and
    /// processes already carry their own bounded timeouts.
    pub const fn none() -> Self {
        Self { expires_at: None }
    }

    pub fn after(duration: Duration) -> Self {
        Self {
            expires_at: Some(Instant::now() + duration),
        }
    }

    pub fn at(expires_at: Instant) -> Self {
        Self {
            expires_at: Some(expires_at),
        }
    }

    pub fn remaining(&self) -> Duration {
        match self.expires_at {
            Some(expires_at) => expires_at.saturating_duration_since(Instant::now()),
            None => Duration::MAX,
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Instant::now() >= expires_at,
            None => false,
        }
    }

    pub fn is_none(&self) -> bool {
        self.expires_at.is_none()
    }

    pub fn check(&self) -> Result<(), String> {
        if self.is_expired() {
            Err("Operation deadline exceeded.".to_string())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
}

impl Default for Cancellation {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn check(&self) -> Result<(), String> {
        if self.is_cancelled() {
            Err("Operation was cancelled.".to_string())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_and_logging_traits_are_object_safe_and_send_sync() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn ProgressSink>();
        assert_send_sync::<dyn LoggerPort>();
        assert_send_sync::<Deadline>();
        assert_send_sync::<Cancellation>();
    }

    #[test]
    fn deadlines_and_cancellation_are_fail_closed() {
        let deadline = Deadline::after(Duration::from_secs(60));
        assert!(!deadline.is_none());
        assert!(!deadline.is_expired());
        assert!(deadline.remaining() <= Duration::from_secs(60));
        assert!(deadline.check().is_ok());

        let expired = Deadline::at(Instant::now() - Duration::from_secs(1));
        assert!(expired.is_expired());
        assert_eq!(
            expired.check().expect_err("expired deadline"),
            "Operation deadline exceeded."
        );

        let cancellation = Cancellation::new();
        assert!(cancellation.check().is_ok());
        cancellation.cancel();
        assert!(cancellation.is_cancelled());
        assert!(cancellation.check().is_err());
    }

    #[test]
    fn non_expiring_deadlines_are_explicit_and_never_fail() {
        let deadline = Deadline::none();
        assert!(deadline.is_none());
        assert!(!deadline.is_expired());
        assert_eq!(deadline.remaining(), Duration::MAX);
        assert!(deadline.check().is_ok());
    }
}