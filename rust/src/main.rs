//! Kiro Crew widget (Rust port): a tray icon with controls for the kirocrew gateway.
//!
//! Everything goes through the `kirocrew` CLI, which is service-aware. Status is a cheap TCP
//! probe of the dashboard port plus a /proc scan, so polling never spawns a process. The tray
//! is a StatusNotifierItem (KDE, GNOME with the AppIndicator extension, XFCE's Status Tray,
//! Cinnamon's XApp applet). Dialogs use `zenity` or `kdialog` and notifications `notify-send`
//! instead of a GUI toolkit, which keeps the binary and its memory footprint small.

use std::env;
use std::fs::{self, File, TryLockError};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

const POLL: Duration = Duration::from_secs(3);
const UNIT: &str = "kirocrew.service";
/// USER_HZ is 100 on every mainstream Linux build; avoids a libc dependency.
const CLK_TCK: f64 = 100.0;

const KIRO: [u8; 3] = [0x90, 0x46, 0xff];
const RED: [u8; 3] = [0xf8, 0x51, 0x49];
const AMBER: [u8; 3] = [0xd2, 0x99, 0x22];

static HANDLE: OnceLock<Handle<Tray>> = OnceLock::new();
/// False while no StatusNotifierWatcher (tray host) is on the session bus.
static TRAY_HOST_UP: AtomicBool = AtomicBool::new(true);

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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dialog {
    Kdialog,
    Zenity,
}

impl Dialog {
    fn program(self) -> &'static str {
        match self {
            Dialog::Kdialog => "kdialog",
            Dialog::Zenity => "zenity",
        }
    }
}

/// kdialog first on KDE, zenity first everywhere else (GNOME, XFCE and Cinnamon are all GTK).
fn pick_dialog(desktop: &str, available: impl Fn(&str) -> bool) -> Option<Dialog> {
    let kde = desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE"));
    let order = if kde {
        [Dialog::Kdialog, Dialog::Zenity]
    } else {
        [Dialog::Zenity, Dialog::Kdialog]
    };
    order.into_iter().find(|d| available(d.program()))
}

fn dialog() -> Option<Dialog> {
    pick_dialog(&env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(), has)
}

/// Ask a yes/no question. `None` means no dialog tool is installed.
fn confirm(text: &str) -> Option<bool> {
    let tool = dialog()?;
    let mut cmd = Command::new(tool.program());
    match tool {
        Dialog::Kdialog => cmd.args(["--title", "Kiro Crew", "--yesno", text]),
        Dialog::Zenity => cmd.args(["--question", "--title", "Kiro Crew", "--text", text]),
    };
    Some(cmd.status().map(|s| s.success()).unwrap_or(false))
}

/// Show a text file in a dialog, or hand it to the default app when no dialog tool exists.
/// Returns true if the file can be deleted now (false when another app may still be reading it).
fn show_text(title: &str, file: &Path) -> bool {
    let Some(tool) = dialog() else {
        let _ = Command::new("xdg-open")
            .arg(file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        return false;
    };
    let mut cmd = Command::new(tool.program());
    match tool {
        Dialog::Kdialog => cmd
            .args(["--title", title, "--textbox"])
            .arg(file)
            .args(["700", "450"]),
        Dialog::Zenity => cmd
            .args([
                "--text-info",
                "--title",
                title,
                "--width=700",
                "--height=450",
                "--filename",
            ])
            .arg(file),
    };
    let _ = cmd.status();
    true
}

fn stop() {
    thread::spawn(|| {
        if !port_open() {
            return;
        }
        match confirm("Stop the gateway?\n\nThis interrupts running Kiro sessions.") {
            Some(true) => {}
            Some(false) => return,
            None => {
                notify("Install zenity or kdialog so Stop can ask for confirmation.");
                return;
            }
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

/// Run a CLI command and show its output in a dialog.
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
        if fs::write(&file, text).is_ok() && show_text(&format!("Kiro Crew · {title}"), &file) {
            let _ = fs::remove_file(&file);
        }
    });
}

struct Tray {
    running: bool,
    busy: bool,
    status: String,
}

impl Tray {
    /// Icon colour, and whether the ghost has its eyes shut (the gateway is stopped).
    fn look(&self) -> ([u8; 3], bool) {
        if self.busy {
            (AMBER, false)
        } else if self.running {
            (KIRO, false)
        } else {
            (RED, true)
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

/// The ghost from Scarlab Icons (MIT, see THIRD_PARTY_NOTICES.md), adapted into a filled body
/// with the face cut out; the asleep variant has its eyes shut. Their only colour is `GHOST_INK`.
const GHOST_AWAKE: &str = include_str!("../assets/ghost.svg");
const GHOST_ASLEEP: &str = include_str!("../assets/ghost-asleep.svg");
const GHOST_INK: &str = "#323232";

/// The tray icon as an ARGB32 pixmap: the ghost drawn in the status colour.
fn ghost(color: [u8; 3], asleep: bool) -> ksni::Icon {
    const N: u32 = 32;
    let ink = format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]);
    let svg = if asleep { GHOST_ASLEEP } else { GHOST_AWAKE }.replace(GHOST_INK, &ink);
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
        .expect("assets/ghost*.svg are valid SVG");
    let scale = N as f32 / tree.size().width();
    let mut pixmap = Pixmap::new(N, N).expect("icon size is non-zero");
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    // SNI wants straight (non-premultiplied) ARGB, tiny-skia stores premultiplied RGBA.
    let data = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.alpha(), c.red(), c.green(), c.blue()]
        })
        .collect();
    ksni::Icon {
        width: N as i32,
        height: N as i32,
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
        let (color, asleep) = self.look();
        vec![ghost(color, asleep)]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: format!("Kiro Crew: {}", self.label()),
            ..Default::default()
        }
    }

    fn watcher_online(&self) {
        TRAY_HOST_UP.store(true, Ordering::Relaxed);
    }

    // Keep running and wait: on GNOME, XFCE and Cinnamon the tray host can start after us.
    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        TRAY_HOST_UP.store(false, Ordering::Relaxed);
        true
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

/// What to do when no StatusNotifier tray host showed up, per desktop.
fn tray_hint(desktop: &str) -> &'static str {
    let d = desktop.to_ascii_uppercase();
    if d.contains("GNOME") || d.contains("UNITY") {
        "No system tray found. Install and enable the \"AppIndicator and KStatusNotifierItem Support\" GNOME Shell extension."
    } else if d.contains("XFCE") {
        "No system tray found. Add the \"Status Tray Plugin\" to the Xfce panel."
    } else if d.contains("CINNAMON") {
        "No system tray found. Install xapp (xapp-sn-watcher) and add the \"XApp Status\" applet to the panel."
    } else if d.contains("KDE") {
        "No system tray found. Check that the System Tray widget is on the panel."
    } else {
        "No StatusNotifier tray found. Use a panel with StatusNotifierItem support (or run snixembed for XEmbed-only trays)."
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
    // assume_sni_available: don't bail out when the tray host isn't up yet (login race).
    let handle = match tray.assume_sni_available(true).spawn() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Could not register the tray icon: {e}");
            std::process::exit(1);
        }
    };
    let _ = HANDLE.set(handle);
    thread::spawn(|| {
        thread::sleep(Duration::from_secs(30));
        if !TRAY_HOST_UP.load(Ordering::Relaxed) {
            let hint = tray_hint(&env::var("XDG_CURRENT_DESKTOP").unwrap_or_default());
            eprintln!("{hint}");
            notify(hint);
        }
    });
    loop {
        refresh();
        thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(programs: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |p| programs.contains(&p)
    }

    #[test]
    fn kde_prefers_kdialog() {
        assert_eq!(
            pick_dialog("KDE", only(&["kdialog", "zenity"])),
            Some(Dialog::Kdialog)
        );
    }

    #[test]
    fn gtk_desktops_prefer_zenity() {
        for de in ["GNOME", "ubuntu:GNOME", "XFCE", "X-Cinnamon", ""] {
            assert_eq!(
                pick_dialog(de, only(&["kdialog", "zenity"])),
                Some(Dialog::Zenity),
                "{de}"
            );
        }
    }

    #[test]
    fn falls_back_to_whatever_is_installed() {
        assert_eq!(pick_dialog("KDE", only(&["zenity"])), Some(Dialog::Zenity));
        assert_eq!(
            pick_dialog("GNOME", only(&["kdialog"])),
            Some(Dialog::Kdialog)
        );
        assert_eq!(pick_dialog("GNOME", only(&[])), None);
    }

    #[test]
    fn ghost_is_drawn_in_the_status_colour() {
        let icon = ghost(KIRO, false);
        assert_eq!((icon.width, icon.height), (32, 32));
        assert_eq!(icon.data.len(), 32 * 32 * 4);
        let px = |x: usize, y: usize| icon.data[(y * 32 + x) * 4..][..4].to_vec();
        // The body is opaque in the status colour, the corner is empty and the eye is cut out.
        assert_eq!(px(4, 16), [0xff, KIRO[0], KIRO[1], KIRO[2]]);
        assert_eq!(px(0, 0)[0], 0);
        assert_eq!(px(12, 13)[0], 0);
        assert_ne!(ghost(RED, false).data, icon.data);
    }

    #[test]
    fn asleep_ghost_has_its_eyes_shut() {
        // Just below an open eye the body is solid; a shut eye is an arc that dips into it.
        let alpha = |icon: ksni::Icon| icon.data[(15 * 32 + 12) * 4];
        assert_eq!(alpha(ghost(RED, false)), 0xff);
        assert!(alpha(ghost(RED, true)) < 0x20);
    }

    #[test]
    fn only_a_stopped_gateway_sleeps() {
        let look = |running, busy| {
            Tray {
                running,
                busy,
                status: String::new(),
            }
            .look()
        };
        assert_eq!(look(false, false), (RED, true));
        assert_eq!(look(true, false), (KIRO, false));
        assert_eq!(look(true, true), (AMBER, false));
        assert_eq!(look(false, true), (AMBER, false));
    }

    #[test]
    fn every_target_desktop_has_a_specific_tray_hint() {
        let generic = tray_hint("");
        for de in ["GNOME", "XFCE", "X-Cinnamon", "KDE"] {
            assert_ne!(tray_hint(de), generic, "{de}");
        }
    }
}
