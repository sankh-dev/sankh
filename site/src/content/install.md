# Install

Sankh comes in two parts that you can install separately:

- **The `sankh` CLI**: `sankh run` for terminals and CI, `sankh serve` for the
  web UI, `sankh import`, `sankh mcp` and the rest. One self-contained binary.
- **The desktop app**: the same UI in a native window. It does not include
  the `sankh` command.

Both read and write the same collections, workspace and trust store, so you
can install either or both.

| OS | CLI | Desktop app |
| --- | --- | --- |
| macOS (Apple silicon, Intel) | install script, Homebrew | Homebrew cask, `.dmg` |
| Linux x86_64 (glibc) | install script, Homebrew | `.deb`, `.rpm`, `.AppImage` |
| Linux x86_64 (musl, e.g. Alpine) | install script | not available |
| Linux arm64 | install script, Homebrew | not available |
| Windows x86_64 | PowerShell installer, `.zip` | `.exe` (setup), `.msi` |

Anything else can [build from source](#build-from-source).

## Requirements

Request files run with `curl` and a POSIX shell:

- **macOS:** nothing to add; `curl` and `sh` ship with the system.
- **Linux:** install `curl` if your distribution leaves it out
  (`sudo apt install curl`, `sudo dnf install curl`, `sudo apk add curl`).
  Many request files also use `jq`.
- **Windows:** install [Git for Windows](https://git-scm.com/download/win)
  for its POSIX shell, and run the CLI from Git Bash. Or use
  [WSL](#windows-subsystem-for-linux) and follow the Linux steps. When
  `curl` is not installed, the Windows build uses a bundled curl.

## CLI

### macOS and Linux

The install script detects your OS and CPU, downloads the matching release
archive from GitHub, verifies its sha256 checksum, and puts `sankh` in
`~/.local/bin`. No root required.

```bash
curl -fsSL https://sankh.dev/install.sh | sh
```

| Variable | Effect |
| --- | --- |
| `SANKH_VERSION` | Version to install, e.g. `0.10.0`. Default: the latest release. |
| `SANKH_INSTALL_DIR` | Install directory. Default: `~/.local/bin`. |

```bash
curl -fsSL https://sankh.dev/install.sh | SANKH_VERSION=0.10.0 sh
curl -fsSL https://sankh.dev/install.sh | SANKH_INSTALL_DIR="$HOME/bin" sh
```

If `~/.local/bin` is not on your `PATH`, the script says so. Add it to your
shell profile (`~/.bashrc`, `~/.zshrc`, ...):

```bash
export PATH="$HOME/.local/bin:$PATH"
```

On Alpine and other musl-based x86_64 systems the script picks the static
musl build automatically.

### Homebrew (macOS, Linux)

```bash
brew install sankh-dev/tap/sankh
```

### Windows

In PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/sankh-dev/sankh/releases/latest/download/sankh-installer.ps1 | iex"
```

The installer puts `sankh.exe` in `%USERPROFILE%\.local\bin` (or
`$env:SANKH_INSTALL_DIR`) and adds that folder to your user `PATH`. Open a new
terminal afterwards. For a specific version, replace `latest/download` with
`download/v0.10.0`.

Then run `sankh` from Git Bash, which is where request files get their shell:

```bash
sankh --version
```

### Windows Subsystem for Linux

Inside a WSL distribution (Ubuntu, Debian, ...), use the Linux install script.
`sankh serve` started in WSL is reachable from the Windows browser at
`http://localhost:4747`.

### Manual download

Every [release](https://github.com/sankh-dev/sankh/releases/latest) has an
archive per platform, each with a `.sha256` file next to it:

| Platform | Archive |
| --- | --- |
| macOS (Apple silicon) | `sankh-aarch64-apple-darwin.tar.xz` |
| macOS (Intel) | `sankh-x86_64-apple-darwin.tar.xz` |
| Linux x86_64 | `sankh-x86_64-unknown-linux-gnu.tar.xz` |
| Linux x86_64 (musl, static) | `sankh-x86_64-unknown-linux-musl.tar.xz` |
| Linux arm64 | `sankh-aarch64-unknown-linux-gnu.tar.xz` |
| Windows x86_64 | `sankh-x86_64-pc-windows-msvc.zip` |

```bash
target=x86_64-unknown-linux-gnu
curl -fsSLO "https://github.com/sankh-dev/sankh/releases/latest/download/sankh-$target.tar.xz"
curl -fsSLO "https://github.com/sankh-dev/sankh/releases/latest/download/sankh-$target.tar.xz.sha256"
sha256sum -c "sankh-$target.tar.xz.sha256"     # macOS: shasum -a 256 -c
tar -xJf "sankh-$target.tar.xz"
install -m 755 "sankh-$target/sankh" ~/.local/bin/sankh
```

### In CI

Use the install script with a pinned version so builds are reproducible. On
GitHub Actions it adds the install directory to `GITHUB_PATH`, so later steps
find `sankh`. See [Running in CI](ci.md) for a full example.

```bash
curl -fsSL https://sankh.dev/install.sh | SANKH_VERSION=0.10.0 sh
```

## Desktop app

Installers and first-launch steps differ per OS. All installers are on the
[latest release](https://github.com/sankh-dev/sankh/releases/latest), each
with a matching `.sha256` file. They are not code signed yet; each section
below says how to get past the warning.

### macOS

With Homebrew (recommended, handles Gatekeeper and upgrades):

```bash
brew install --cask sankh-dev/tap/sankh-desktop
```

Or download the disk image for your Mac
([Apple silicon](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-macos-aarch64.dmg),
[Intel](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-macos-x64.dmg)),
open it and drag **Sankh** to **Applications**. The first time, right-click
the app and choose **Open**, or run:

```bash
xattr -dr com.apple.quarantine /Applications/Sankh.app
```

### Windows

Download [Sankh-windows-setup.exe](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-windows-setup.exe)
(per-user install) or
[Sankh-windows-x64.msi](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-windows-x64.msi)
(for managed or machine-wide installs) and run it. In the SmartScreen prompt
choose **More info**, then **Run anyway**.

The app needs [Git for Windows](https://git-scm.com/download/win) for a POSIX
shell. It finds a standard Git install even when Git Bash is not on `PATH`.
When `curl` is not installed, the app uses a bundled curl. Requests run in
the background and do not open a terminal.

### Debian, Ubuntu, Linux Mint, Pop!_OS

[Sankh-linux-amd64.deb](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-amd64.deb)

```bash
curl -fsSLO https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-amd64.deb
sudo apt install ./Sankh-linux-amd64.deb
```

Pin a version when the install must be reproducible:

```bash
curl -fsSLO https://github.com/sankh-dev/sankh/releases/download/v0.10.0/Sankh_0.10.0_amd64.deb
sudo apt install ./Sankh_0.10.0_amd64.deb
```

`apt` pulls in WebKitGTK and the other libraries the app needs.

### Fedora, RHEL, CentOS Stream, openSUSE

[Sankh-linux-x86_64.rpm](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-x86_64.rpm)

```bash
curl -fsSLO https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-x86_64.rpm
sudo dnf install ./Sankh-linux-x86_64.rpm       # Fedora, RHEL, CentOS Stream
sudo zypper install ./Sankh-linux-x86_64.rpm    # openSUSE
```

Pin a version when the install must be reproducible:

```bash
curl -fsSLO https://github.com/sankh-dev/sankh/releases/download/v0.10.0/Sankh-0.10.0-1.x86_64.rpm
sudo dnf install ./Sankh-0.10.0-1.x86_64.rpm       # Fedora, RHEL, CentOS Stream
sudo zypper install ./Sankh-0.10.0-1.x86_64.rpm    # openSUSE
```

### Any other Linux (AppImage)

The AppImage runs on most x86_64 distributions (Arch, Manjaro, Void, ...)
without installing:

[Sankh-linux-amd64.AppImage](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-amd64.AppImage)

```bash
curl -fsSLO https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-amd64.AppImage
chmod +x Sankh-linux-amd64.AppImage
./Sankh-linux-amd64.AppImage
```

Pin a version when the install must be reproducible:

```bash
curl -fsSLO https://github.com/sankh-dev/sankh/releases/download/v0.10.0/Sankh_0.10.0_amd64.AppImage
chmod +x Sankh_0.10.0_amd64.AppImage
./Sankh_0.10.0_amd64.AppImage
```

It uses the system WebKitGTK (`libwebkit2gtk-4.1`), which most desktop
distributions already have. If it does not start, install it:

```bash
sudo apt install libwebkit2gtk-4.1-0      # Debian, Ubuntu
sudo dnf install webkit2gtk4.1            # Fedora
sudo pacman -S webkit2gtk-4.1             # Arch
```

Some distributions also need FUSE 2 for AppImages (`libfuse2` on Ubuntu
22.04+, `fuse2` on Arch).

There is no desktop build for Linux arm64 yet. Use the CLI's `sankh serve`
and a browser instead.

## Verify

```bash
sankh --version
```

Then follow the [quickstart](quickstart.md) to create and run a first
collection.

## Upgrade

| Installed with | Upgrade with |
| --- | --- |
| Install script | Run the same `curl ... \| sh` command again |
| Homebrew | `brew upgrade sankh` and `brew upgrade --cask sankh-desktop` |
| PowerShell installer | Run the same PowerShell command again |
| `.deb`, `.rpm`, `.exe`, `.msi` | Install the newer file over the old one |
| AppImage | Replace the file with the newer one |

New releases and their changes are listed in the
[changelog](https://github.com/sankh-dev/sankh/blob/main/CHANGELOG.md).

## Uninstall

Remove the CLI:

```bash
rm ~/.local/bin/sankh                     # install script
brew uninstall sankh                      # Homebrew
```

On Windows, delete `%USERPROFILE%\.local\bin\sankh.exe`.

Remove the desktop app:

```bash
brew uninstall --cask sankh-desktop       # Homebrew
sudo apt remove sankh                     # .deb
sudo dnf remove sankh                     # .rpm
```

On macOS without Homebrew, drag **Sankh** from **Applications** to the Trash.
On Windows, use **Settings > Apps**. An AppImage is just the file.

Uninstalling leaves your collections alone. Sankh's own settings (the saved
workspace, trust store and Scratch) are shared by the CLI and the app and
stay too; delete them by hand if you want a clean slate:

| OS | Settings and Scratch |
| --- | --- |
| Linux | `~/.config/sankh` and `~/.local/share/sankh` |
| macOS | `~/Library/Application Support/sankh` |
| Windows | `%APPDATA%\sankh` |

`SANKH_CONFIG_DIR` and `SANKH_DATA_DIR` override these locations.

## Build from source

Needs Rust (stable) and Node 22. The web UI is built first, because both the
CLI and the app embed it.

```bash
git clone https://github.com/sankh-dev/sankh
cd sankh
(cd frontend && npm ci && npm run build)
cargo install --path crates/sankh-cli
```

The desktop app is built with the Tauri CLI. On Linux, install the WebKitGTK
development packages first (on Debian and Ubuntu:
`sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`):

```bash
cd crates/sankh-desktop
npx @tauri-apps/cli@^2 build
```

The installers land in `target/release/bundle/`.
