# Desktop app

The desktop app is the Sankh web UI in a native window. It opens the same
saved [workspace](workspaces.md) as `sankh serve`, edits the same `.sh`
files, and runs requests the same way, so you can switch between the app,
the browser, and `sankh run` at any time.

## Download

On macOS, install with Homebrew:

```bash
brew install --cask sankh-dev/tap/sankh-desktop
```

The cask clears the quarantine flag on install, so the unsigned app opens
without a Gatekeeper prompt. `brew upgrade` picks up new releases.

Or download the installer for your platform. These links always point at the
latest release:

| Platform | Download |
| --- | --- |
| macOS (Apple silicon) | [Sankh-macos-aarch64.dmg](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-macos-aarch64.dmg) |
| macOS (Intel) | [Sankh-macos-x64.dmg](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-macos-x64.dmg) |
| Windows | [Sankh-windows-setup.exe](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-windows-setup.exe) or [Sankh-windows-x64.msi](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-windows-x64.msi) |
| Linux | [Sankh-linux-amd64.AppImage](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-amd64.AppImage) |
| Linux (Debian, Ubuntu) | [Sankh-linux-amd64.deb](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-amd64.deb) |
| Linux (Fedora, RHEL, openSUSE) | [Sankh-linux-x86_64.rpm](https://github.com/sankh-dev/sankh/releases/latest/download/Sankh-linux-x86_64.rpm) |

The release also has the versioned filename (`Sankh_<version>_...`) and a
matching `.sha256` checksum next to each file. There is no desktop
build for Linux arm64 yet. [Install](install.md#desktop-app) has
copy-paste commands for each OS and Linux distribution, and how to upgrade and
uninstall.

The installers are not code signed yet:

- **macOS (DMG):** right-click the app and choose **Open** the first time, or run
  `xattr -dr com.apple.quarantine /Applications/Sankh.app`. Not needed with
  the Homebrew cask.
- **Windows:** in the SmartScreen prompt choose **More info**, then
  **Run anyway**.
- **Linux:** make the AppImage executable (`chmod +x Sankh_*.AppImage`). The
  app uses the system WebKitGTK (`libwebkit2gtk-4.1`), which most desktop
  distributions already have.

The app does not include the `sankh` command. Install the CLI separately (see
[Install](install.md#cli)) if you also want `sankh run` in a terminal or in
CI.

## Requirements

None. Like the CLI, the app sends requests with its own HTTP client; it needs
neither `curl` nor a shell.

Apps started from a launcher, Finder or the Start menu do not see variables
set in your shell profile, such as `HTTPS_PROXY` or secrets exported there.
Put request variables in the collection's `.env.local` instead.

## What is different from `sankh serve`

- **Folder picker.** **Open folder…** and **Import from Postman…** (for the
  destination of the new collection) have a **Choose...** button that opens
  the native folder dialog. See [Import from Postman](import.md#from-the-app).
- **One window.** Launching the app again focuses the window that is already
  open.
- **No fixed port.** The app has no `--port`, `--listen` or `--token`
  options. To share a UI with another machine, use `sankh serve` instead (see
  [Web UI](serve.md)).

If the saved workspace cannot be opened (for example a malformed
`workspace.toml`), the app opens with only Scratch and says why. Folders you
add in that state are kept for the session only.

## How it works

The app embeds the same server as `sankh serve`. At startup it binds a random
port on `127.0.0.1`, generates a token for that launch, and loads the UI from
that address. Every API call carries the token, and the server's usual checks
apply: loopback only, foreign `Host` headers rejected, cross-origin calls
rejected. Requests execute on your machine, not in the window.

The window can call exactly one native function, the folder dialog. It has no
other access to your system.
