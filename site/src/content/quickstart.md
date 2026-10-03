# Quickstart

## Install

```bash
curl -fsSL https://sankh.dev/install.sh | sh                 # latest
curl -fsSL https://sankh.dev/install.sh | SANKH_VERSION=0.2.0 sh
```

The installer downloads the release archive for your platform from GitHub,
verifies its sha256 checksum, and installs `sankh` into `~/.local/bin` (override
with `SANKH_INSTALL_DIR`). No root required.

Other options:

```bash
brew install sankh-dev/tap/sankh
```

```powershell
powershell -c "irm https://github.com/sankh-dev/sankh/releases/latest/download/sankh-installer.ps1 | iex"
```

Or build from source (needs Rust and Node 22):

```bash
(cd frontend && npm ci && npm run build)
cargo install --path crates/sankh-cli
```

Sankh needs `curl` and a POSIX shell at run time. On Windows use Git Bash or WSL.

## Desktop app

The desktop app is the same web UI in a native window, with a native folder
picker. It runs the server in-process on a random loopback port behind a
per-launch token, and uses the same saved workspace as `sankh serve`. Download
it from the [latest release](https://github.com/sankh-dev/sankh/releases/latest):

| Platform | File |
| --- | --- |
| macOS (Apple silicon) | `Sankh_<version>_aarch64.dmg` |
| macOS (Intel) | `Sankh_<version>_x64.dmg` |
| Windows | `Sankh_<version>_x64-setup.exe` or `.msi` |
| Linux | `Sankh_<version>_amd64.AppImage` or `.deb` |

The installers are not code signed yet:

- **macOS:** right-click the app and choose Open the first time, or run
  `xattr -dr com.apple.quarantine /Applications/Sankh.app`.
- **Windows:** in the SmartScreen prompt choose More info, then Run anyway.
- **Linux:** the AppImage needs `chmod +x`; the app uses the system
  WebKitGTK (`libwebkit2gtk-4.1`).

The desktop app still needs `curl` and a POSIX shell (Git Bash on Windows; it
finds a standard Git for Windows install even when it is not on `PATH`).

## Your first collection

```bash
sankh init my-api              # scaffold a collection
sankh trust my-api             # request files are scripts: trust before running
sankh run my-api --env dev     # run everything, exit non-zero on failure
sankh serve my-api             # web UI on http://localhost:4747
```

A collection is a plain folder of `.sh` files, each holding one `curl` command:

```bash
#!/usr/bin/env bash
# @name Create pet
# @tags smoke
# @expect status 201
# @expect json .name == "Rex"
# @capture PET_ID=.id
curl -sS "$BASE_URL/pets" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Rex"}'
```

Every file still runs on its own, with or without Sankh:

```bash
set -a; . environments/dev.env; set +a
sh pets/02-create.sh
```

The full file format, including every annotation, is described in
[Collection format](format.md).

## Try the bundled example

The repository ships a petstore collection and an in-memory mock server:

```bash
cargo run --example petstore_mock &        # http://localhost:4010
cp examples/petstore/.env.example examples/petstore/.env.local
sankh trust examples/petstore
sankh run examples/petstore
```

## How it runs a file

Sankh never modifies a request file. It sources the file in a shell (`bash` if
the shebang names it, else `sh`) after defining a `curl` function that adds
`-o`, `-D` and `-w '%{json}'` to record the body, headers and timing. If a
raw-mode file calls curl several times, the last response is checked.

## Next steps

- [Running in CI](ci.md): flags, reports and exit codes.
- [Web UI](serve.md): editing and running requests in the browser.
- [Trust and secrets](trust-secrets.md): what Sankh will run and what it hides.
