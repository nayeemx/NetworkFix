use crate::mode::Mode;
use crate::report::{FixReport, Step};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum ProgressEvent {
    Start { mode: Mode },
    Step(Step),
    Message { text: String },
    Done { report: FixReport },
}

impl ProgressEvent {
    pub fn message(text: impl Into<String>) -> Self {
        ProgressEvent::Message { text: text.into() }
    }
}
