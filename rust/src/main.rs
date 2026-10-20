//! Kiro Crew widget (Rust port): a tray icon with controls for the kirocrew gateway.
//!
//! Same behaviour as `kiro_widget.py`. Everything goes through the `kirocrew` CLI, which is
//! service-aware. Status is a cheap TCP probe of the dashboard port plus a /proc scan, so
//! polling never spawns a process. Dialogs and notifications use `kdialog` / `notify-send`
//! instead of a GUI toolkit, which keeps the binary and its memory footprint small.

use std::env;
use std::fs::{self, File, TryLockError};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};

const POLL: Duration = Duration::from_secs(3);
const UNIT: &str = "kirocrew.service";
/// USER_HZ is 100 on every mainstream Linux build; avoids a libc dependency.
const CLK_TCK: f64 = 100.0;

const GREEN: [u8; 3] = [0x3f, 0xb9, 0x50];
const RED: [u8; 3] = [0xf8, 0x51, 0x49];
const AMBER: [u8; 3] = [0xd2, 0x99, 0x22];

static HANDLE: OnceLock<Handle<Tray>> = OnceLock::new();

fn port() -> u16 {
    env::var("KIROCREW_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5476)
}

fn kirocrew() -> PathBuf {
    if let Some(path) = env::var_os("PATH") {
        for dir in env::split_paths(&path) {
            let candidate = dir.join("kirocrew");
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(env::var("HOME").unwrap_or_default()).join(".local/bin/kirocrew")
}

fn has(program: &str) -> bool {
    env::var_os("PATH")
        .map(|p| env::split_paths(&p).any(|d| d.join(program).is_file()))
        .unwrap_or(false)
}

fn port_open() -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port()));
    TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

/// PID of the `kirocrew gateway` process, found by scanning /proc.
fn gateway_pid() -> Option<u32> {
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(raw) = fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let argv: Vec<&[u8]> = raw.split(|b| *b == 0).collect();
        if argv.iter().take(3).any(|a| a.ends_with(b"kirocrew")) && argv.contains(&&b"gateway"[..])
        {
            return Some(pid);
        }
    }
    None
}

fn uptime_of(pid: u32) -> String {
    let secs = (|| {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let start: f64 = stat
            .rsplit_once(')')?
            .1
            .split_whitespace()
            .nth(19)?
            .parse()
            .ok()?;
        let boot: f64 = fs::read_to_string("/proc/uptime")
            .ok()?
            .split_whitespace()
            .next()?
            .parse()
            .ok()?;
        Some((boot - start / CLK_TCK) as u64)
    })();
    let Some(secs) = secs else { return "?".into() };
    let (d, h, m) = (secs / 86400, secs % 86400 / 3600, secs % 3600 / 60);
    let mut parts = Vec::new();
    if d > 0 {
        parts.push(format!("{d}d"));
    }
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    parts.push(format!("{m}m"));
    parts.join(" ")
}

fn unit_installed() -> bool {
    Command::new("systemctl")
        .args(["cat", UNIT])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn notify(msg: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "Kiro Crew", "Kiro Crew", msg])
        .status();
}

fn update(f: impl FnOnce(&mut Tray)) {
    if let Some(h) = HANDLE.get() {
        h.update(f);
    }
}

/// Re-read gateway state into the tray. Leaves the amber "working" state alone while busy.
fn refresh() {
    let running = port_open();
    let pid = if running { gateway_pid() } else { None };
    let status = if running {
        match pid {
            Some(pid) => format!("Running · pid {pid} · up {}", uptime_of(pid)),
            None => format!("Running · port {}", port()),
        }
    } else {
        "Stopped".into()
    };
    update(|t| {
        t.running = running;
        t.status = status;
    });
}

/// Mark the tray busy and poll until the port reaches the wanted state or 30s pass.
fn wait_for(up: bool, ok_msg: &str, fail_msg: &str) {
    update(|t| t.busy = true);
    let deadline = Instant::now() + Duration::from_secs(30);
    let reached = loop {
        if port_open() == up {
            break true;
        }
        if Instant::now() > deadline {
            break false;
        }
        thread::sleep(Duration::from_millis(500));
    };
    update(|t| t.busy = false);
    refresh();
    notify(if reached { ok_msg } else { fail_msg });
}

fn start() {
    thread::spawn(|| {
        if port_open() {
            return;
        }
        // Prefer the systemd unit when installed; otherwise run a detached foreground gateway.
        let mut cmd = if unit_installed() && has("pkexec") {
            let mut c = Command::new("pkexec");
            c.args(["systemctl", "start", UNIT]);
            c
        } else {
            let mut c = Command::new(kirocrew());
            c.args(["gateway", "--no-open"]);
            c
        };
        let spawned = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
        if spawned.is_err() {
            notify("Could not launch the gateway.");
            return;
        }
        wait_for(
            true,
            "Gateway is up.",
            "Gateway did not come up within 30s. Check Logs / Doctor.",
        );
    });
}

fn stop() {
    thread::spawn(|| {
        if !port_open() {
            return;
        }
        let confirmed = Command::new("kdialog")
            .args([
                "--title",
                "Kiro Crew",
                "--yesno",
                "Stop the gateway?\n\nThis interrupts running Kiro sessions.",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        // The unit has Restart=always, so signalling the process (what `kirocrew stop` does for a
        // non-root user) just makes systemd bring it back. Only an explicit `systemctl stop` is
        // exempt from Restart=, and that needs privilege, hence pkexec.
        let mut cmd = if unit_installed() && has("pkexec") {
            let mut c = Command::new("pkexec");
            c.args(["systemctl", "stop", UNIT]);
            c
        } else {
            let mut c = Command::new(kirocrew());
            c.arg("stop");
            c
        };
        update(|t| t.busy = true);
        let result = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match result {
            Ok(s) if s.success() => wait_for(
                false,
                "Gateway stopped.",
                "Gateway still answering after stop.",
            ),
            other => {
                update(|t| t.busy = false);
                refresh();
                let code = other
                    .ok()
                    .and_then(|s| s.code())
                    .map_or("?".into(), |c| c.to_string());
                notify(&format!(
                    "Stop failed (exit {code}); the gateway is still running."
                ));
            }
        }
    });
}

fn open_dashboard() {
    thread::spawn(|| {
        // `kirocrew token` prints an authenticated URL; it is opened, never shown or logged.
        let out = Command::new(kirocrew()).arg("token").output();
        let fallback = format!("http://localhost:{}", port());
        let url = out
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(str::trim)
                    .find(|l| l.starts_with("http"))
                    .map(String::from)
            })
            .unwrap_or(fallback);
        let _ = Command::new("xdg-open")
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

/// Run a CLI command and show its output in a kdialog text box.
fn show_cli(title: &'static str, args: &'static [&'static str]) {
    thread::spawn(move || {
        let mut text = format!("$ kirocrew {}\n", args.join(" "));
        if let Ok(o) = Command::new(kirocrew()).args(args).output() {
            // The CLI prints config-deprecation WARNING noise on every call; drop it.
            for stream in [&o.stdout, &o.stderr] {
                for line in String::from_utf8_lossy(stream).lines() {
                    if !line.contains(" WARNING kiro_crew.config.loader") {
                        text.push_str(line);
                        text.push('\n');
                    }
                }
            }
            if let Some(code) = o.status.code().filter(|c| *c != 0) {
                text.push_str(&format!("[exit {code}]\n"));
            }
        } else {
            text.push_str("Could not run kirocrew.\n");
        }
        let dir = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir);
        let file = dir.join(format!(
            "kiro-widget-{}-{}.txt",
            std::process::id(),
            title.replace(' ', "_")
        ));
        if fs::write(&file, text).is_ok() {
            let _ = Command::new("kdialog")
                .args(["--title", &format!("Kiro Crew · {title}"), "--textbox"])
                .arg(&file)
                .args(["700", "450"])
                .status();
            let _ = fs::remove_file(Path::new(&file));
        }
    });
}

struct Tray {
    running: bool,
    busy: bool,
    status: String,
}

impl Tray {
    fn color(&self) -> [u8; 3] {
        if self.busy {
            AMBER
        } else if self.running {
            GREEN
        } else {
            RED
        }
    }

    fn label(&self) -> String {
        if self.busy {
            "Working…".into()
        } else {
            self.status.clone()
        }
    }
}

/// A filled, antialiased circle as an ARGB32 icon.
fn dot(color: [u8; 3]) -> ksni::Icon {
    const N: i32 = 32;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    let (c, r) = (N as f32 / 2.0, N as f32 * 0.375);
    for y in 0..N {
        for x in 0..N {
            let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            let alpha = ((r - d + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
            data.extend_from_slice(&[alpha, color[0], color[1], color[2]]);
        }
    }
    ksni::Icon {
        width: N,
        height: N,
        data,
    }
}

fn item(label: &str, enabled: bool, activate: fn()) -> MenuItem<Tray> {
    StandardItem {
        label: label.into(),
        enabled,
        activate: Box::new(move |_| activate()),
        ..Default::default()
    }
    .into()
}

impl ksni::Tray for Tray {
    // Left click opens the same menu as right click.
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "kiro-widget".into()
    }

    fn title(&self) -> String {
        "Kiro Crew".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![dot(self.color())]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: format!("Kiro Crew: {}", self.label()),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let idle = !self.busy;
        vec![
            StandardItem {
                label: format!("Kiro Crew: {}", self.label()),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            item("Go aboard", self.running, open_dashboard),
            item("Start", idle && !self.running, start),
            item("Stop", idle && self.running, stop),
            MenuItem::Separator,
            item("Logs", true, || show_cli("Logs", &["logs", "-n", "100"])),
            item("Doctor", true, || show_cli("Doctor", &["doctor"])),
            item("Cron jobs", self.running, || {
                show_cli("Cron jobs", &["cron", "list"])
            }),
            item("Subagents", self.running, || {
                show_cli("Subagents", &["spawn", "list"])
            }),
            MenuItem::Separator,
            item("Quit widget", true, || std::process::exit(0)),
        ]
    }
}

/// Hold an exclusive lock for the process lifetime so autostart plus a manual launch can't
/// leave two tray icons. The lock dies with the process, so a crash never leaves it stale.
fn single_instance() -> Option<File> {
    let dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let file = File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join("kiro-widget.lock"))
        .ok()?;
    match file.try_lock() {
        Ok(()) => Some(file),
        Err(TryLockError::WouldBlock) => None,
        // Can't lock (odd filesystem): better a possible duplicate than no widget.
        Err(TryLockError::Error(_)) => Some(file),
    }
}

fn main() {
    let Some(_lock) = single_instance() else {
        eprintln!("kiro-widget is already running.");
        return;
    };
    let tray = Tray {
        running: false,
        busy: false,
        status: "Checking…".into(),
    };
    let handle = match tray.spawn() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Could not register the tray icon: {e}");
            std::process::exit(1);
        }
    };
    let _ = HANDLE.set(handle);
    loop {
        refresh();
        thread::sleep(POLL);
    }
}
