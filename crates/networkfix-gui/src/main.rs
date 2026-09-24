use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use networkfix_core::elevation::{is_elevated, relaunch_elevated, Elevation};
use networkfix_core::{
    run_fix, run_fix_with, CommandOutput, MockRunner, Mode, ProgressEvent, StepStatus,
};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

const CSS: &str = r#"
:root { color-scheme: dark; }
body { margin:0; font-family: 'Segoe UI', system-ui, sans-serif; background:#0f1115; color:#e6e6e6; }
.wrap { padding: 20px; max-width: 720px; margin: 0 auto; }
h1 { font-size: 1.3rem; margin: 0 0 4px; }
.sub { color:#9aa0a6; font-size: .9rem; margin-bottom: 16px; }
.grid { display:grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-bottom: 14px; }
button.mode { background:#1b2330; color:#e6e6e6; border:1px solid #2e3a4d; border-radius:8px; padding:14px 12px; font-size:1rem; cursor:pointer; text-align:left; }
button.mode:hover:not(:disabled) { background:#243044; }
button.mode:disabled { opacity:.5; cursor:not-allowed; }
button.mode .title { display:block; }
button.mode .hint { display:block; color:#9aa0a6; font-size:.75rem; margin-top:4px; }
button.elev { background:#2b3a1f; border:1px solid #4a6b2f; color:#c6f0a0; border-radius:6px; padding:8px 12px; cursor:pointer; }
button.elev:disabled { opacity:.5; cursor:not-allowed; }
.banner { background:#3a2e14; border:1px solid #7a5c1e; color:#f0d78c; padding:10px 12px; border-radius:8px; margin-bottom:14px; display:flex; justify-content:space-between; align-items:center; gap:10px; }
pre.log { background:#0a0c10; border:1px solid #1e2530; border-radius:8px; padding:12px; height: 260px; overflow:auto; font-size: .82rem; white-space: pre-wrap; }
.ok { color:#7ee787; } .warn { color:#e3b341; } .fail { color:#ff7b72; } .skip { color:#8b949e; }
.status { margin-top:10px; color:#9aa0a6; font-size:.85rem; }
"#;

#[derive(Clone, PartialEq)]
struct LogLine {
    text: String,
    cls: &'static str,
}

enum UiMsg {
    Line(LogLine),
    Done(String),
}

fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let t = secs % 86_400;
    format!("{:02}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
}

fn log_line(text: String, cls: &'static str) -> UiMsg {
    UiMsg::Line(LogLine {
        text: format!("[{}] {text}", timestamp()),
        cls,
    })
}

fn format_event(ev: ProgressEvent) -> UiMsg {
    match ev {
        ProgressEvent::Start { mode } => log_line(format!("== {mode} =="), ""),
        ProgressEvent::Message { text } => log_line(format!(".. {text}"), ""),
        ProgressEvent::Step(s) => {
            let (cls, tag) = match s.status {
                StepStatus::Ok => ("ok", "OK"),
                StepStatus::Warn => ("warn", "WARN"),
                StepStatus::Fail => ("fail", "FAIL"),
                StepStatus::Skipped => ("skip", "SKIP"),
            };
            log_line(format!("[{tag}] {} — {}", s.name, s.detail), cls)
        }
        ProgressEvent::Done { report } => UiMsg::Done(format!(
            "Done in {} ms (exit {})",
            report.duration_ms,
            report.exit_code()
        )),
    }
}

fn dry_run_enabled() -> bool {
    std::env::var("NETWORKFIX_DRY_RUN")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn start_fix(
    mode: Mode,
    label: &str,
    tx: UnboundedSender<UiMsg>,
    mut running: Signal<bool>,
    mut status: Signal<String>,
) {
    if *running.peek() {
        return;
    }
    running.set(true);
    status.set(format!("Running {label}…"));
    let dry_run = dry_run_enabled();
    std::thread::spawn(move || {
        let emit = |ev: ProgressEvent| {
            let _ = tx.send(format_event(ev));
        };
        if dry_run {
            let dry_line = log_line(".. dry-run: no real commands will be executed".into(), "");
            let _ = tx.send(dry_line);
            let mut mock = MockRunner::new();
            mock.set_default(CommandOutput::ok_empty());
            let _ = run_fix_with(mode, &mock, false, &emit);
        } else {
            let _ = run_fix(mode, emit);
        }
    });
}

type FixChannel = Rc<(
    UnboundedSender<UiMsg>,
    RefCell<Option<UnboundedReceiver<UiMsg>>>,
)>;

fn main() {
    let window = WindowBuilder::new()
        .with_title("NetworkFix")
        .with_inner_size(LogicalSize::new(640.0, 480.0));
    let cfg = Config::new().with_window(window);
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(App);
}

#[component]
fn App() -> Element {
    let elevated = use_signal(is_elevated);
    let mut running = use_signal(|| false);
    let mut log = use_signal(Vec::<LogLine>::new);
    let mut status = use_signal(|| String::from("Ready"));

    let channel: FixChannel = use_hook(|| {
        let (tx, rx) = unbounded_channel::<UiMsg>();
        Rc::new((tx, RefCell::new(Some(rx))))
    });

    let channel_future = Rc::clone(&channel);
    use_future(move || {
        let channel = Rc::clone(&channel_future);
        async move {
            let Some(mut rx) = channel.1.borrow_mut().take() else {
                return;
            };
            while let Some(msg) = rx.recv().await {
                match msg {
                    UiMsg::Line(line) => log.write().push(line),
                    UiMsg::Done(text) => {
                        status.set(text);
                        running.set(false);
                    }
                }
            }
            running.set(false);
        }
    });

    let status_text = status.read().clone();
    let elev_label = if *elevated.read() { "yes" } else { "no" };
    let os = std::env::consts::OS;
    let version = networkfix_core::VERSION;
    let tx_q = channel.0.clone();
    let tx_f = channel.0.clone();
    let tx_h = channel.0.clone();
    let tx_n = channel.0.clone();

    rsx! {
        style { "{CSS}" }
        div { class: "wrap",
            h1 { "NetworkFix" }
            div { class: "sub", "Hotspot & cellular repair — no reboot for most cases" }

            if !*elevated.read() {
                div { class: "banner",
                    span { "Not running as administrator/root — restart elevated for full effect." }
                    button {
                        class: "elev",
                        disabled: *running.read(),
                        onclick: move |_| {
                            match std::env::current_exe() {
                                Ok(exe) => match relaunch_elevated(&exe, &[]) {
                                    Ok(Elevation::RelaunchRequested) => std::process::exit(0),
                                    Ok(_) => status.set(String::from(
                                        "Elevation was not granted — run the app as administrator.",
                                    )),
                                    Err(e) => status.set(format!("Relaunch failed: {e}")),
                                },
                                Err(e) => status.set(format!("Cannot locate executable: {e}")),
                            }
                        },
                        "Restart elevated"
                    }
                }
            }

            div { class: "grid",
                button {
                    class: "mode",
                    disabled: *running.read(),
                    onclick: move |_| start_fix(Mode::Quick, "Quick", tx_q.clone(), running, status),
                    span { class: "title", "1 · Quick" }
                    span { class: "hint", "Restart cellular + hotspot services, flush DNS" }
                }
                button {
                    class: "mode",
                    disabled: *running.read(),
                    onclick: move |_| start_fix(Mode::Full, "Full", tx_f.clone(), running, status),
                    span { class: "title", "2 · Full" }
                    span { class: "hint", "Quick + adapters + stack reset" }
                }
                button {
                    class: "mode",
                    disabled: *running.read(),
                    onclick: move |_| start_fix(Mode::HotspotOnly, "Hotspot", tx_h.clone(), running, status),
                    span { class: "title", "3 · Hotspot only" }
                    span { class: "hint", "Wi-Fi + hotspot services only" }
                }
                button {
                    class: "mode",
                    disabled: *running.read(),
                    onclick: move |_| start_fix(Mode::NoInternet, "No internet", tx_n.clone(), running, status),
                    span { class: "title", "4 · No internet" }
                    span { class: "hint", "Clients joined but have no internet (ICS/NAT)" }
                }
            }

            pre { class: "log",
                for line in log.read().iter() {
                    span { class: line.cls, "{line.text}\n" }
                }
            }
            div { class: "status", "{status_text}" }
            div { class: "status", "OS: {os} · elevated: {elev_label} · v{version}" }
        }
    }
}
