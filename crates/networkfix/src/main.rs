use clap::{Parser, Subcommand};
use networkfix_core::elevation::{is_elevated, relaunch_elevated, Elevation};
use networkfix_core::{run_fix, FixReport, Mode, ProgressEvent, StepStatus};
use std::io::{self, BufRead, Write};

#[derive(Parser)]
#[command(
    name = "networkfix",
    version,
    about = "Cross-platform network hotspot/cellular repair"
)]
struct Args {
    /// Emit FixReport JSON on stdout (progress on stderr)
    #[arg(long, global = true)]
    json: bool,

    /// Skip UAC/pkexec relaunch (operations may warn without admin)
    #[arg(long, global = true)]
    no_elevate: bool,

    #[command(subcommand)]
    mode: Option<ModeCmd>,
}

#[derive(Subcommand)]
enum ModeCmd {
    /// Restart hotspot + network services, flush DNS (fastest)
    Quick,
    /// Quick + adapter cycle + stack reset
    Full,
    /// Hotspot/Wi-Fi services only
    Hotspot,
    /// Fix ICS/NAT/forwarding for clients with no internet
    NoInternet,
}

impl From<ModeCmd> for Mode {
    fn from(c: ModeCmd) -> Self {
        match c {
            ModeCmd::Quick => Mode::Quick,
            ModeCmd::Full => Mode::Full,
            ModeCmd::Hotspot => Mode::HotspotOnly,
            ModeCmd::NoInternet => Mode::NoInternet,
        }
    }
}

fn menu_select() -> Result<Mode, String> {
    eprintln!("============================================");
    eprintln!(" Network Fix Menu - pick an option");
    eprintln!("============================================");
    eprintln!(" [1] Quick  - restart cellular + hotspot services (fastest)");
    eprintln!(" [2] Full   - also Wi-Fi, stack reset, deeper clean");
    eprintln!(" [3] Hotspot only - restart Wi-Fi + hotspot, keep cellular");
    eprintln!(" [4] Hotspot has clients but NO internet (ICS/NAT fix)");
    eprintln!(" [Q] Quit");
    eprint!("Choose (1/2/3/4/Q): ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    match line.trim().to_ascii_uppercase().as_str() {
        "1" => Ok(Mode::Quick),
        "2" => Ok(Mode::Full),
        "3" => Ok(Mode::HotspotOnly),
        "4" => Ok(Mode::NoInternet),
        "Q" | "" => Err("quit".into()),
        other => Err(format!("unknown mode: {other}")),
    }
}

fn main() {
    let args = Args::parse();
    let mode = match args.mode {
        Some(m) => m.into(),
        None => match menu_select() {
            Ok(m) => m,
            Err(e) if e == "quit" => std::process::exit(0),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        },
    };

    if !args.no_elevate && !is_elevated() {
        eprintln!("Requesting administrator privileges...");
        let exe = std::env::current_exe().expect("current exe");
        let mut forward = vec![mode.to_string()];
        if args.json {
            forward.push("--json".into());
        }
        match relaunch_elevated(&exe, &forward) {
            Ok(Elevation::RelaunchRequested) => {
                std::process::exit(0);
            }
            _ => {
                eprintln!("error: elevation denied — re-run as admin/root or pass --no-elevate");
                std::process::exit(3);
            }
        }
    }

    let json = args.json;
    let report = run_fix(mode, |ev| match ev {
        ProgressEvent::Start { mode } => eprintln!("== mode {mode} =="),
        ProgressEvent::Message { text } => eprintln!(".. {text}"),
        ProgressEvent::Step(s) => {
            let tag = match s.status {
                StepStatus::Ok => "OK  ",
                StepStatus::Warn => "WARN",
                StepStatus::Fail => "FAIL",
                StepStatus::Skipped => "SKIP",
            };
            eprintln!("[{tag}] {} — {}", s.name, s.detail);
        }
        ProgressEvent::Done { .. } => {}
    });

    emit_final(&report, json);
    std::process::exit(report.exit_code() as i32);
}

fn emit_final(report: &FixReport, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(report).expect("serialize")
        );
    } else {
        eprintln!(
            "Done in {} ms — exit {}",
            report.duration_ms,
            report.exit_code()
        );
    }
}
