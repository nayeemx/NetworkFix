use crate::mode::Mode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Ok,
    Warn,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub status: StepStatus,
    pub detail: String,
}

impl Step {
    pub fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: StepStatus::Ok,
            detail: detail.into(),
        }
    }
    pub fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: StepStatus::Warn,
            detail: detail.into(),
        }
    }
    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: StepStatus::Fail,
            detail: detail.into(),
        }
    }
    pub fn skipped(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: StepStatus::Skipped,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixReport {
    pub mode: Mode,
    pub steps: Vec<Step>,
    pub elevated: bool,
    pub duration_ms: u64,
}

impl FixReport {
    pub fn new(mode: Mode, elevated: bool) -> Self {
        Self {
            mode,
            steps: Vec::new(),
            elevated,
            duration_ms: 0,
        }
    }

    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn worst(&self) -> Option<StepStatus> {
        if self.steps.iter().any(|s| s.status == StepStatus::Fail) {
            Some(StepStatus::Fail)
        } else if self.steps.iter().any(|s| s.status == StepStatus::Warn) {
            Some(StepStatus::Warn)
        } else {
            None
        }
    }

    pub fn exit_code(&self) -> u32 {
        match self.worst() {
            Some(StepStatus::Fail) => 2,
            Some(StepStatus::Warn) => 1,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_priority_fail_over_warn() {
        let mut r = FixReport::new(Mode::Quick, true);
        r.steps.push(Step::ok("a", ""));
        assert_eq!(r.exit_code(), 0);

        let mut r = FixReport::new(Mode::Quick, false);
        r.steps.push(Step::skipped("b", "no wwan"));
        assert_eq!(r.exit_code(), 0);

        let mut r = FixReport::new(Mode::Quick, false);
        r.steps.push(Step::warn("c", "slow"));
        r.steps.push(Step::fail("d", "x"));
        assert_eq!(r.exit_code(), 2);

        let mut r = FixReport::new(Mode::Quick, false);
        r.steps.push(Step::warn("c", "slow"));
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn report_json_roundtrip() {
        let mut r = FixReport::new(Mode::NoInternet, true);
        r.steps.push(Step::ok("forwarding", "enabled"));
        let s = serde_json::to_string(&r).unwrap();
        let back: FixReport = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }
}
