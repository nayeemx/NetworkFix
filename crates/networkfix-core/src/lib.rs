pub mod mode;
pub mod report;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use mode::Mode;
pub use report::{FixReport, Step, StepStatus};
