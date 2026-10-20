# Installing the system dependencies

Commands for the common package managers; see [Requirements](README.md#requirements) for what each tool does. The `.deb`, `.rpm` and Arch packages pull in the runtime tools themselves. In each pair the second line is the build tools, needed only to build from source.

    # Debian / Ubuntu (no pkexec package on your release? use policykit-1)
    sudo apt install zenity libnotify-bin xdg-utils pkexec
    sudo apt install build-essential curl

    # Fedora
    sudo dnf install zenity libnotify xdg-utils polkit
    sudo dnf install gcc curl

    # Arch Linux
    sudo pacman -S zenity libnotify xdg-utils polkit
    sudo pacman -S base-devel curl

    # openSUSE
    sudo zypper install zenity libnotify-tools xdg-utils polkit
    sudo zypper install gcc curl

    # NixOS, in configuration.nix (the module provides the setuid pkexec wrapper)
    security.polkit.enable = true;
    environment.systemPackages = with pkgs; [ zenity libnotify xdg-utils ];

Rust comes from [rustup](https://rustup.rs): distro `rustc` is often older than the 1.89 required (Ubuntu 24.04 ships 1.75).

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Optional: `kdialog` for native KDE dialogs (Nix: `kdePackages.kdialog`), and `gnome-shell-extension-appindicator` for the GNOME tray (Nix: `gnomeExtensions.appindicator`; the same name on apt, dnf, pacman and zypper). Enable the extension in the Extensions app and log out and back in; Ubuntu preinstalls it. Tray hosts for Xfce and Cinnamon are under [Supported desktops](README.md#supported-desktops).
