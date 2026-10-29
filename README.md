# Sankh

**Blow the conch. Run your APIs.**

Sankh is a very lightweight request manager. A collection is a plain folder of
`.sh` files, each holding one `curl` command. One binary runs them in CI
(`sankh run`) or serves a small web UI (`sankh serve`) where the server, not
the browser, sends the requests.

Sankh reads the curl command and sends it with its own HTTP client. It never
runs a shell or `curl`, so a collection from someone else cannot execute
anything, and it works the same on Linux, macOS and Windows.

> Your API collection is a git repo that runs anywhere. The UI is optional.

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

Every file is still a curl command that runs on its own, with or without Sankh:

```bash
set -a; . environments/dev.env; set +a
sh pets/02-create.sh
```

## Install

```bash
curl -fsSL https://sankh.dev/install.sh | sh                 # latest
curl -fsSL https://sankh.dev/install.sh | SANKH_VERSION=0.10.1 sh
```

Or build from source (needs Rust and Node 22):

```bash
(cd frontend && npm ci && npm run build)
cargo install --path crates/sankh-cli
```

Sankh has no run-time dependencies: no shell, no `curl`.

### Desktop app

The same UI in a native window: download the `.dmg` (macOS), `-setup.exe` or
`.msi` (Windows), or `.AppImage`, `.deb` or `.rpm` (Linux) from the
[latest release](https://github.com/sankh-dev/sankh/releases/latest). It runs
the server in-process on a random loopback port behind a per-launch token and
opens your saved workspace, like `sankh serve`.

The installers are not code signed yet. On macOS right-click the app and
choose Open the first time (or `xattr -dr com.apple.quarantine
/Applications/Sankh.app`); on Windows choose More info, then Run anyway.

## Quick start

```bash
sankh init my-api              # scaffold a collection
sankh run my-api --check       # check every file without sending anything
sankh run my-api --env dev     # run everything, exit non-zero on failure
sankh serve my-api             # web UI on http://localhost:4747
sankh workspace add my-api     # or keep it, so plain `sankh serve` opens it
```

Try the bundled example against an in-memory mock:

```bash
cargo run --example petstore_mock &        # http://localhost:4010
cp examples/petstore/.env.example examples/petstore/.env.local
sankh run examples/petstore
```

## `sankh run`

```
sankh run [PATH] [--env NAME] [--folder F]... [--tag T]... [--all]
          [--check] [--report junit|json] [--report-file PATH|-] [--bail] [-v]
```

- `PATH` is a request file or folder (default `.`). Folders run in order:
  numeric prefix (`01-`), then name; override with `[order]` in `sankh.toml`.
- `--folder` and `--tag` are repeatable; a request matches if it is in **any**
  listed folder and has **any** listed tag.
- Before sending anything, every selected file is parsed and its variables and
  `@file` paths are checked. If any file has an error, all of them are
  reported, compiler style, and nothing is sent. `--check` stops there.
- `--report-file -` writes the report to stdout and human output to stderr.

| Exit code | Meaning |
| --- | --- |
| 0 | All requests passed |
| 1 | An `@expect` or `@capture` failed |
| 2 | A file could not be read or built: unsupported flag, shell syntax, undefined variable, missing `@file`, bad URL; or a usage error |
| 3 | A request could not be sent: DNS, connection, TLS, timeout |

When several kinds of failure happen in one run, the highest number wins
in the order 2, 3, 1.

### CI

```yaml
- run: curl -fsSL https://sankh.dev/install.sh | sh
- run: sankh run . --env ci --folder smoke --report junit
  env:
    TOKEN: ${{ secrets.API_TOKEN }}
```

## `sankh serve`

```
sankh serve [PATH]... [--listen 127.0.0.1] [--port 4747] [--token T] [--allow-host H]...
            [--allow-target T]... [--deny-target T]...
```

Without a path, `sankh serve` opens your saved [workspace](#workspaces). With
one or more paths, it opens just those folders for this session.

The UI has a file tree, form and raw (CodeMirror) editors, an environment
picker, live results for single requests and whole folders, and import from
curl or a Postman export (**Add collection > Import from Postman…** in the header). Values captured in the UI (e.g. a login token) are kept per environment
for the session, so you can run requests one at a time; **Clear captures** in
a collection's **⋯** menu forgets them. **Manage environments…** in the same
menu edits `environments/*.env` and `.env.local` (create, rename, delete, set default).

Security:

- Binds to `127.0.0.1` by default and rejects foreign `Host` headers (DNS rebinding).
- Any other `--listen` address requires `--token` (or `SANKH_TOKEN`). The
  printed URL carries the token in the fragment (`#token=…`), which never
  reaches server logs.
- Cross-origin API calls are rejected; paths cannot leave the collection root.
- Prefer an SSH tunnel (`ssh -L 4747:localhost:4747 host`) or a TLS reverse
  proxy (`--allow-host api-tools.example.com`) for remote use.
- Anyone who can use the UI can make the server send requests, including to
  hosts on its own network. On a shared server, limit where requests may go
  with `--allow-target` and `--deny-target` (host names, `*.suffix`, IP
  addresses or CIDR ranges, repeatable; deny wins). They are checked again on
  every redirect.

## Workspaces

A workspace is the set of collections the UI shows side by side, so you can
work on a users API and a payments API at the same time without switching.
Collections stay independent: each has its own environment picker, captured
values and runs.

```
sankh workspace add PATH...          # keep folders in the workspace
sankh workspace list [--json]        # show them
sankh workspace remove ID|PATH       # unlink (alias: unlink); files stay on disk
```

- The saved workspace lives in `~/.config/sankh/workspace.toml` (or
  `$SANKH_CONFIG_DIR`). `sankh serve` with no path opens it; folders added or
  unlinked in the UI are saved there too.
- `sankh serve A B` opens a session-only workspace with A and B. Changes made
  in the UI are not saved.
- **Scratch** is a built-in collection that is always listed first, for
  trying requests without an existing folder. It lives in
  `~/.local/share/sankh/scratch` (or `$SANKH_DATA_DIR/scratch`) and cannot
  be unlinked. Use **Copy to collection…** in the editor's **⋯** menu to move
  a request from Scratch into a real collection; existing files are never
  overwritten.
- A missing folder stays in the list, marked missing, until you unlink it.
  Collections cannot be nested inside each other.

## `sankh import`

```
sankh import postman COLLECTION.json [--env ENV.json]... [-o DIR] [--force] [--json]
```

Converts a Postman Collection v2.0/v2.1 export into a Sankh folder: folders
become directories, requests become numbered `.sh` files, and collection and
environment variables become `environments/*.env` (`{{baseUrl}}` becomes
`${BASE_URL}`). Auth becomes explicit headers or `-u`. Secret values and
literal credentials are never written to disk; they are listed in
`.env.example`, together with variables that requests use but nothing
defines, and `.env.local` is created with sample values for you to replace
(an existing one is kept).

Common test-script statements (`pm.response.to.have.status(201)`,
`pm.environment.set("token", pm.response.json().token)`, simple
`pm.expect(...).to.eql(...)`) become `@expect` and `@capture`. Anything else
is kept as comments in the file and listed in the import report, because
Sankh does not run JavaScript. Import never writes into a non-empty folder
without `--force`.

The web UI and desktop app can do the same from **Add collection > Import from Postman…**: pick
the export, review the import report, choose a destination, and the new
collection is added to the workspace.

## AI agents and MCP

Agents with a shell can use `sankh list --json` and
`sankh run --report json --report-file -` directly. Point them at
[sankh.dev/llms.txt](https://sankh.dev/llms.txt) or copy
[`skills/sankh`](skills/sankh/SKILL.md) into `.cursor/skills/` or
`.claude/skills/` to teach them the format.

Any MCP client can use the built-in stdio server:

```json
{ "mcpServers": { "sankh": { "command": "sankh", "args": ["mcp"] } } }
```

```
sankh mcp [PATH]...
```

Without a path it exposes the saved workspace plus Scratch; with paths, only
those folders. Tools: `list_collections`, `list_requests`, `show_request`,
`list_environments` and `run`. It is read and run only: it never edits files.
Like `sankh run`, it checks every selected file first and sends nothing if one
has an error. Secrets are masked as in `sankh run`.

## Collections

```
my-api/
  sankh.toml            # optional: name, default_env, timeout, max_body, env_proxy, [order]
  .env.example
  .gitignore            # includes .env.local
  environments/
    dev.env
    ci.env
  auth/01-login.sh
  users/01-list.sh
```

Environment layers, lowest to highest priority: `environments/<name>.env`,
`.env.local`, the process environment, then values captured earlier in the run.

The full format, including every annotation, is in [docs/format.md](docs/format.md).

## Secrets and safety

- Values of variables whose names contain `TOKEN`, `SECRET`, `KEY`,
  `PASSWORD`, `AUTH` or `COOKIE` are replaced with `***` in all output,
  reports and UI responses, including captured values. The request echo also
  masks `Authorization`, `Proxy-Authorization` and `Cookie` headers.
- Keep secrets in `.env.local` (gitignored) or CI secrets, never in
  `environments/*.env`.
- Values never leave the process: no shell, no command line, nothing in `ps`.
- `@file` paths (`-d @body.json`, `-F file=@logo.png`, `--cacert`) must stay
  inside the collection folder.
- `-k` prints a warning once per run, and the UI shows a banner on the
  response.

## How it runs a file

Sankh never modifies a request file, and never executes it. It reads the
annotations, splits the curl command with POSIX quoting rules, expands
`$VAR`, `${VAR}` and `${VAR:-default}` from the environment (an undefined
variable is an error), and sends the request with its own HTTP client
(rustls; system certificates). Pipes, redirections, `$(...)`, `&&` and other
shell syntax are errors, as is any curl flag it does not support. The full
list is in [curl flags](https://sankh.dev/docs/curl-flags).

Responses are capped at 10 MB (`max_body` in `sankh.toml`), requests time out
after 30 s (`timeout`, `# @timeout` or `-m`), and `-L` follows at most 50
redirects.

## Development

```bash
cargo test                              # unit, snapshot, CLI and server tests
(cd frontend && npm run dev)            # UI dev server, proxies /api to :4747
cargo run -- serve examples/petstore    # API for the dev server
cargo run -p sankh-desktop              # desktop app (needs frontend/dist built)
```

The desktop app (`crates/sankh-desktop`, Tauri 2) is left out of plain
`cargo build`/`cargo test` because on Linux it needs `webkit2gtk-4.1` and
`libsoup-3.0` development packages. Build installers with
`cd crates/sankh-desktop && cargo tauri build`.

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Telemetry

None. Sankh makes no network requests other than the ones in your files.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

*Sankh (शंख, "shankh") is the conch shell blown to announce a beginning.*
