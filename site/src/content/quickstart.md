# Quickstart

## Install

macOS and Linux:

```bash
curl -fsSL https://sankh.dev/install.sh | sh     # or: brew install sankh-dev/tap/sankh
```

Windows (PowerShell), then use `sankh` from Git Bash:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/sankh-dev/sankh/releases/latest/download/sankh-installer.ps1 | iex"
```

Sankh needs `curl` and a POSIX shell at run time. On Windows use Git Bash or WSL.

Prefer a native window? The [desktop app](desktop.md) is the same UI as
`sankh serve`, for macOS, Windows and Linux. [Install](install.md) has every
option for the CLI and the app per OS (Homebrew, `.deb`, `.rpm`, AppImage,
`.msi`, manual download, building from source), plus upgrading and
uninstalling.

## Your first collection

```bash
sankh init my-api              # scaffold a collection
sankh trust my-api             # request files are scripts: trust before running
sankh run my-api --env dev     # run everything, exit non-zero on failure
sankh serve my-api             # web UI on http://localhost:4747
sankh workspace add my-api     # or keep it, so plain `sankh serve` opens it
```

Already have a Postman collection? Convert it with
`sankh import postman collection.json -o my-api` (see
[Import from Postman](import.md)).

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
[Collection format](format.md). For a walkthrough of `@expect`, `@capture`
and chaining requests into a flow, see
[Assertions and chaining](assertions.md).

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

- [Assertions and chaining](assertions.md): check responses and pass values between requests.
- [Running in CI](ci.md): flags, reports and exit codes.
- [Web UI](serve.md): editing and running requests in the browser.
- [Desktop app](desktop.md): the same UI in a native window.
- [Workspaces](workspaces.md): several collections side by side, and Scratch.
- [Import from Postman](import.md): convert an existing collection.
- [Trust and secrets](trust-secrets.md): what Sankh will run and what it hides.
