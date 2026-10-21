# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1] - 2026-10-21

First tagged version.

### Added

- System tray widget for the Kiro Crew gateway (Rust, `ksni` over D-Bus). The ghost icon shows the state: Kiro purple when running, amber when working, red with its eyes shut when stopped.
- Gateway status from a port probe on 5476 (`KIROCREW_PORT` to override), with PID and uptime.
- Start and Stop. Stop asks first, since it interrupts running Kiro sessions. Both go through `pkexec systemctl start|stop kirocrew.service` when that unit exists, else a detached `kirocrew gateway --no-open` / `kirocrew stop`.
- "Go aboard" opens the dashboard using `kirocrew token`; the token URL is never displayed.
- Logs, Doctor, Cron jobs and Subagents entries, with output shown in a dialog.
- Single-instance lock in `$XDG_RUNTIME_DIR`.
- Support for KDE Plasma, GNOME (AppIndicator extension), Xfce and Cinnamon: `zenity` or `kdialog` dialogs, and a wait for a late tray host with a desktop-specific hint after 30 seconds.
- Size-tuned release build (a 2.3 MB binary that idles at about 5 MB of RAM).
- Packages: Arch `PKGBUILD` and AUR `kiro-widget-git` recipe, `.deb` (`cargo-deb`) and `.rpm` (`cargo-generate-rpm`), each with a start-menu entry and an XDG autostart entry.
- CI (fmt, clippy, tests, build, packages), Dependabot, MIT license.
- README Requirements and Installation sections, and the versioning policy and release steps.

[Unreleased]: https://github.com/PatiHox/kiro-widget/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/PatiHox/kiro-widget/releases/tag/v0.0.1
