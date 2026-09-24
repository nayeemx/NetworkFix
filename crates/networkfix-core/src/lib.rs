pub mod elevation;
pub mod mode;
pub mod progress;
pub mod report;
pub mod runner;
#[cfg(any(target_os = "windows", test))]
pub mod windows;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use elevation::{is_elevated, relaunch_elevated, Elevation};
pub use mode::Mode;
pub use progress::ProgressEvent;
pub use report::{FixReport, Step, StepStatus};
pub use runner::{CommandOutput, CommandRunner, SystemRunner};
