//! Framework-neutral typed ports shared by API and implementation crates.
//!
//! The traits in this module are object-safe and use display-ready `String`
//! errors so callers do not depend on a host or provider error framework.

pub mod control;
pub mod network;
pub mod process;
pub mod workspace;

pub use control::{
    Cancellation, Deadline, LogLevel, LoggerPort, NoopLogger, NoopProgress, ProgressSink,
    ProgressUpdate,
};
pub use network::{DownloadRequest, NetworkPort, NetworkRequest, ReqwestNetwork, ServiceLabel};
pub use process::{
    JavaRunRequest, JavaRunner, OutputLimits, ProcessOutput, ProcessPort, ProcessRequest,
};
pub use workspace::{ArchiveEntry, ArchiveLimits, PathPort, SafeWorkspace};