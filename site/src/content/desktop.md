# Desktop app

The desktop app is the Sankh web UI in a native window. It opens the same
saved [workspace](workspaces.md) as `sankh serve`, edits the same `.sh`
files, and runs requests the same way, so you can switch between the app,
the browser, and `sankh run` at any time.

## Download

Get the installer for your platform from the
[latest release](https://github.com/sankh-dev/sankh/releases/latest):

| Platform | File |
| --- | --- |
| macOS (Apple silicon) | `Sankh_<version>_aarch64.dmg` |
| macOS (Intel) | `Sankh_<version>_x64.dmg` |
| Windows | `Sankh_<version>_x64-setup.exe` or `Sankh_<version>_x64_en-US.msi` |
| Linux | `Sankh_<version>_amd64.AppImage` or `Sankh_<version>_amd64.deb` |

Each file has a matching `.sha256` checksum next to it.

The installers are not code signed yet:

- **macOS:** right-click the app and choose **Open** the first time, or run
  `xattr -dr com.apple.quarantine /Applications/Sankh.app`.
- **Windows:** in the SmartScreen prompt choose **More info**, then
  **Run anyway**.
- **Linux:** make the AppImage executable (`chmod +x Sankh_*.AppImage`). The
  app uses the system WebKitGTK (`libwebkit2gtk-4.1`), which most desktop
  distributions already have.

The app does not include the `sankh` command. Install the CLI separately (see
the [quickstart](quickstart.md)) if you also want `sankh run` in a terminal or
in CI.

## Requirements

Like the CLI, the app runs request files with `curl` and a POSIX shell. On
Windows that means [Git for Windows](https://git-scm.com/download/win): the
app finds a standard Git install even when Git Bash is not on `PATH`.

Apps started from a launcher, Finder or the Start menu do not inherit your
terminal's `PATH`. On macOS and Linux the app reads `PATH` from your login
shell at startup, so tools your request files call (for example `jq`) are
found as they are in a terminal.

## What is different from `sankh serve`

- **Folder picker.** **Add folder** has a **Choose...** button that opens the
  native folder dialog.
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
