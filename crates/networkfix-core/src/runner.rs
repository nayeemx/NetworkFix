use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status == 0
    }
    pub fn ok_empty() -> Self {
        Self {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        }
    }
}

pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String>;
}

pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        Ok(CommandOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

pub struct MockRunner {
    scripts: HashMap<String, CommandOutput>,
    sequences: RefCell<HashMap<String, VecDeque<CommandOutput>>>,
    default: Option<CommandOutput>,
    pub calls: RefCell<Vec<(String, Vec<String>)>>,
}

impl MockRunner {
    pub fn new() -> Self {
        Self {
            scripts: HashMap::new(),
            sequences: RefCell::new(HashMap::new()),
            default: None,
            calls: RefCell::new(Vec::new()),
        }
    }
    pub fn expect(&mut self, program: &str, output: CommandOutput) {
        self.scripts.insert(program.to_string(), output);
    }
    pub fn expect_seq(&mut self, program: &str, outputs: Vec<CommandOutput>) {
        self.sequences
            .borrow_mut()
            .insert(program.to_string(), outputs.into_iter().collect());
    }
    pub fn set_default(&mut self, output: CommandOutput) {
        self.default = Some(output);
    }
}

impl Default for MockRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRunner for MockRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        self.calls.borrow_mut().push((
            program.to_string(),
            args.iter().map(|s| s.to_string()).collect(),
        ));
        {
            let mut sequences = self.sequences.borrow_mut();
            if let Some(queue) = sequences.get_mut(program) {
                if queue.len() > 1 {
                    if let Some(o) = queue.pop_front() {
                        return Ok(o);
                    }
                } else if let Some(o) = queue.front() {
                    return Ok(o.clone());
                }
            }
        }
        if let Some(o) = self.scripts.get(program) {
            return Ok(o.clone());
        }
        if let Some(d) = &self.default {
            return Ok(d.clone());
        }
        Err(format!("no mock for {program}"))
    }
}

pub type EventHandler<'a> = &'a dyn Fn(crate::progress::ProgressEvent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_records_calls_and_returns_scripted_output() {
        let mut m = MockRunner::new();
        m.expect(
            "netsh",
            CommandOutput {
                status: 0,
                stdout: "ok".into(),
                stderr: String::new(),
            },
        );
        let out = m.run("netsh", &["winsock", "reset"]).unwrap();
        assert!(out.success());
        assert_eq!(m.calls.borrow().len(), 1);
        assert_eq!(m.calls.borrow()[0].0, "netsh");
    }

    #[test]
    fn mock_fails_when_no_script_and_no_default() {
        let m = MockRunner::new();
        assert!(m.run("ipconfig", &["/flushdns"]).is_err());
    }

    #[test]
    fn mock_default_covers_unscripted_programs() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        });
        assert!(m.run("anything", &[]).unwrap().success());
    }

    #[test]
    fn expect_seq_returns_outputs_in_order_then_sticks_to_last() {
        let mut m = MockRunner::new();
        m.expect_seq(
            "nmcli",
            vec![
                CommandOutput {
                    status: 0,
                    stdout: "first".into(),
                    stderr: String::new(),
                },
                CommandOutput {
                    status: 1,
                    stdout: "second".into(),
                    stderr: String::new(),
                },
            ],
        );
        assert_eq!(m.run("nmcli", &[]).unwrap().stdout, "first");
        assert_eq!(m.run("nmcli", &[]).unwrap().stdout, "second");
        assert_eq!(m.run("nmcli", &[]).unwrap().stdout, "second");
        assert_eq!(m.calls.borrow().len(), 3);
    }

    #[test]
    fn expect_seq_takes_priority_over_expect_and_default() {
        let mut m = MockRunner::new();
        m.expect(
            "nmcli",
            CommandOutput {
                status: 0,
                stdout: "from-expect".into(),
                stderr: String::new(),
            },
        );
        m.set_default(CommandOutput {
            status: 0,
            stdout: "from-default".into(),
            stderr: String::new(),
        });
        m.expect_seq(
            "nmcli",
            vec![CommandOutput {
                status: 0,
                stdout: "from-seq".into(),
                stderr: String::new(),
            }],
        );
        assert_eq!(m.run("nmcli", &[]).unwrap().stdout, "from-seq");
        assert_eq!(m.run("nmcli", &[]).unwrap().stdout, "from-seq");
    }
}
