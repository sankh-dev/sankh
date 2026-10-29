# Web UI

## `sankh serve`

```
sankh serve [PATH]... [--listen 127.0.0.1] [--port 4747] [--token T] [--allow-host H]...
            [--allow-target T]... [--deny-target T]...
```

Without a path, `sankh serve` opens your saved [workspace](workspaces.md),
plus Scratch. With one or more paths, it opens just those folders for this
session. Prefer a native window? The [desktop app](desktop.md) is the same UI.

The UI has a file tree for each collection, form and raw (CodeMirror)
editors, an environment picker per collection, live results for single
requests and whole folders, and import from curl or from a Postman export
(see [Import from Postman](import.md#from-the-app)). **Add collection > Open
folder…** in the header puts another collection next to the others; **Copy to
collection…** in the editor's **⋯** menu copies a request between collections. The server, not the browser, executes the requests, so there are
no CORS workarounds and the results match `sankh run` exactly.

Values captured in the UI (for example a login token) are kept per collection
and environment for the session, so you can run requests one at a time. **Clear captures**
in the collection's **⋯** menu forgets them for the selected environment.

Edits made in the UI are written back to the same `.sh` files, so a change in
the browser is a normal diff in git. Changes made on disk (in your editor, or
by `git pull`) show up in the UI without a reload.

## Working in the UI

### Collections and files

- Each collection has **+** (new request), **Run** (the whole collection) and
  a **⋯** menu: **Manage environments…**, **Import Postman environments…**,
  **Clear captures**, **Collapse all** / **Expand all** folders, **Open
  folder** (shows it in your file manager; only when the server runs on your
  machine) and **Unlink**.
- Open and closed folders, and the widths of the file tree, editor and
  response columns, are remembered across reloads. Drag a column border to
  resize it (or focus it and use the arrow keys; double-click resets).

### Editing a request

- **Form** and **Raw** modes edit the same file. Form mode has the name,
  description and tags as an inline header, the method and URL with **Save**
  and **Run** beside them, and tabs for headers, body, assertions, captures
  and settings (including a per-request **Timeout**, saved as `@timeout`).
- The **Body** tab's **Format** button indents a JSON body, keeping bare
  `$VAR` references intact.
- Typing `$` or `${` in the URL, headers, body, curl flags, assertions or
  captures suggests variables from the environment, `.env.local` and
  captures, with their values.
- Hovering a `$VAR` shows its value and where it comes from, with an
  **Edit** button. Variables that are empty, still `changeme`, or missing
  from the selected environment show as amber or red chips under the URL
  (underlined in Raw mode) and are counted in the top bar; click one to set
  it in the environment file or `.env.local`.
- **Copy as curl**, **Copy to collection…** and **Delete file** are in the
  editor's **⋯** menu.

### Running and responses

- While a request or folder runs, **Run** becomes **Stop** (or press
  **Esc**). The request in flight is killed and the rest are marked skipped;
  captures from requests that finished are kept.
- JSON responses are a collapsible tree. Hover a key for **Expect** and
  **Capture**: they prefill an `@expect json` or `@capture` line with the jq
  path, an operator and the current value, and **Add** puts it in the open
  request. Save to keep it. See [Assertions and chaining](assertions.md).
- Image responses (up to 2 MB) are shown as images, HTML is rendered in a
  sandboxed preview that runs no scripts and loads nothing external, and XML
  is indented. **Preview** / **Raw** switches back to the raw text.

### Environments and secrets

**Manage environments…** edits `environments/*.env` and the `.env.local`
overrides: add, edit and remove variables, and create, duplicate, rename,
delete or set the default environment. Comments and `${VAR}` references are
kept as written. Secret values have an eye toggle and a copy button, and
**Reveal secrets** shows them all; values still set to `changeme` are
highlighted.

Run results in the UI show real values, including secrets, because the UI is
yours and runs on your machine. `sankh run`, its reports and `sankh mcp`
still mask them (see [Secrets and safety](secrets-safety.md)).

## Security model

- Binds to `127.0.0.1` by default and rejects foreign `Host` headers, which
  blocks DNS rebinding.
- Any other `--listen` address requires `--token` (or `SANKH_TOKEN`). The
  printed URL carries the token in the fragment (`#token=...`), which never
  reaches server logs.
- Cross-origin API calls are rejected; paths cannot leave the collection root.
- Request files never execute anything; the server sends the request itself.
- Before a run sends anything, every selected file is checked. If one has an
  error, the run is blocked, each error is shown with its line and column, and
  nothing is sent.
- The server sends requests from its own machine, so anyone who can use the UI
  can reach hosts on its network. When others can reach the server, limit
  targets with `--allow-target` and `--deny-target` (host names, `*.suffix`,
  IP addresses or CIDR ranges, repeatable; deny wins; checked on every
  redirect). See [Secrets and safety](secrets-safety.md#sending-requests-from-a-shared-server).

## Remote use

Prefer an SSH tunnel, which keeps the server bound to localhost:

```bash
ssh -L 4747:localhost:4747 host
# then, on the remote host:
sankh serve my-api
```

Or put it behind a TLS reverse proxy and allow the public host name:

```bash
sankh serve my-api --listen 0.0.0.0 --token "$SANKH_TOKEN" \
  --allow-host api-tools.example.com
```
