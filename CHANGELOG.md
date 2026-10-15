# Changelog

## [Unreleased]

## [0.9.0] - 2026-10-15

### Added

- **Pick assertions and captures from the response.** The JSON response is a
  collapsible tree; hover a key for **Expect** and **Capture** buttons. They
  open a small form prefilled with the jq path, an operator and the current
  value (or a capture name such as `ACCESS_TOKEN`), with a preview of the
  `# @expect` / `# @capture` line. **Add** puts it in the open request, in
  Form or Raw mode; save to keep it.
- **Import Postman environments into an existing collection.** A
  collection's **⋯** menu has **Import Postman environments…**, which
  previews the variables added, updated and unchanged per environment before
  writing the env files; secrets can go to `.env.local`. Keys are renamed
  the same way as a full Postman import. API:
  `POST /api/c/{id}/envs/import-postman`.
- **Variable tooltips.** Hovering `$VAR` in the URL, headers or raw mode
  shows its value and where it comes from, with an **Edit** button. The
  quick "set variable" popover can save to the selected environment file or
  to `.env.local`.
- **Collapse and expand all folders.** A collection's **⋯** menu collapses
  or expands every folder, and open/closed folders are remembered across
  reloads.

### Changed

- **Secrets are shown in the web UI.** Run results in the browser (response
  body, headers, captures, assertion messages and the log) show real values
  instead of `***`. `sankh run`, its reports and the MCP server still mask
  them.

## [0.8.0] - 2026-10-13

### Added

- **Variables without a value are flagged.** A variable that is empty, still
  `changeme`, or not defined in the selected environment shows as an amber or
  red chip under the URL, is underlined in raw mode, and is counted in the top
  bar. Click a chip (or hover the underline) to set it in `.env.local`
  without leaving the request. Postman imports now fill placeholders with
  `changeme`. API: `GET /api/c/{id}/envs/{name}` reports `unset` per
  variable and `refs` with the status of every name the collection uses.
- **Variable suggestions.** Typing `$` or `${` in the URL, headers, body,
  curl flags, assertions or captures lists variables from the environment,
  `.env.local` and captures, with their values. Raw mode does the same and
  closes the brace.
- **Format JSON.** The **Body** tab has a **Format** button that indents JSON
  bodies, keeping bare `$VAR` references intact.

### Changed

- **Request editor layout.** The name, description and tags are an inline
  header instead of form fields, and headers, body, assertions, captures and
  settings are tabs with counts. The method is a pill joined to the URL, with
  **Save** and **Run** right beside it; the **Form** / **Raw** switch sits
  before the name. The top bar shows the collection and file path.
- **Secrets.** Token, password and key values have an eye toggle inside the
  field and a copy button. The environment manager has **Reveal secrets** and
  highlights values still set to `changeme`.

### Fixed

- Switching between **Form** and **Raw** (and saving) works when the UI runs
  from the Vite dev server.

## [0.7.0] - 2026-10-09

### Added

- **Image, HTML and XML previews.** The response **Body** tab now shows
  `image/*` responses as images (up to 2 MB, with their pixel size), renders
  HTML in a sandboxed preview, and indents XML the way it already did JSON.
  A **Preview** / **Raw** toggle switches back to the raw text. The HTML preview
  runs no scripts and loads no external CSS, images or fonts, so pages may look
  plain. API: results include `body_base64` for image responses; `sankh run
  --report json` and the MCP server leave it out.

## [0.6.1] - 2026-10-09

### Added

- **Open folder.** A collection's **⋯** menu has **Open folder**, which shows
  the collection in your file manager. Only offered when the server runs on
  your machine (loopback); `GET /api/info` reports it as `can_reveal`. API:
  `POST /api/c/{id}/reveal`.

### Fixed

- A collection's **⋯** menu no longer draws underneath the next collection's
  header.
- Collections no longer randomly show "No requests yet." when several load or
  refresh at once (for example right after a Postman import).

## [0.6.0] - 2026-10-06

### Added

- **Stop a running request or folder run.** While a run is in progress the
  editor's **Run** button becomes **Stop**, the run panel has a **Stop**
  button, and **Esc** stops it too. The request in flight is killed (with
  everything its shell started) and the rest are marked skipped; captures
  from requests that finished are kept. API: `POST /api/runs/{id}/cancel`.
  Results have a new `cancelled` outcome and summaries a `cancelled` count.
- **Per-request `@timeout`.** `# @timeout 500ms`, `10s`, `2m` (or a bare
  number of seconds) overrides `timeout` in `sankh.toml` for that request,
  and the form has a **Timeout** field. A timeout fails with
  `timed out after <duration>`.
- **Resizable columns.** Drag the borders between the file explorer, editor
  and response (or focus one and use the arrow keys; double-click resets).
  Widths are remembered.

### Changed

- **Multi-line descriptions.** **Description** is now a text area. Each line
  is saved as its own `# @description` line and joined back when read.
- **The Tags field explains itself:** tags pick requests in CI with
  `sankh run --tag smoke`.
- Timeouts and stops now kill the whole process group on Unix, so a `curl`
  started by the request file can no longer outlive its run.
- **Postman import asks where to create the collection.** The dialog now
  shows a **Where to create it** section right after the file pickers:
  **Save in** (a parent folder you can browse in the web UI too, not only in
  the desktop app) and **Folder name**, with the full path it will create, and
  the button names the folder (`Import to orders-api/`).
- **Imports default to `~/sankh-collections/<name>`** instead of a new folder
  directly in your home folder. The folder is created on the first import.

## [0.5.0] - 2026-10-06

### Added

- **`sankh mcp`: an MCP server for AI agents.** Add
  `{"command": "sankh", "args": ["mcp"]}` to any MCP client to expose the
  saved workspace (or `sankh mcp PATH...` for just those folders) over stdio.
  Tools: `list_collections`, `list_requests`, `show_request`,
  `list_environments` and `run`. It is read and run only: it never edits
  files and never grants trust (`SANKH_TRUST` does not apply), secrets are
  masked as in `sankh run`, and response bodies are truncated to 16 KB.
- **Duplicate environment.** In **Manage environments…**, **Duplicate**
  creates a copy of the selected environment's variables under a suggested
  name (`staging-copy`, then `staging-copy-2`, …) that you can edit first.

### Changed

- **Trust only lapses when the collection changes.** Moving git HEAD (a pull,
  or checking out a teammate's branch) no longer revokes trust if no file
  under the trusted folder differs from the trusted commit. When files did
  change, the error, the app banner and the MCP message list them. If git
  can't compute the diff, trust still lapses.
- **Refreshed app UI with one place for each action.** The header now has a
  single **Add collection** menu (Open folder…, Import from Postman…); the
  duplicate sidebar buttons and the header's **New request** and **Clear
  captures** are gone. Each collection has **+** (new request), **Run** and a
  **⋯** menu with **Manage environments…**, **Clear captures** and **Unlink**,
  so these always act on that collection. The editor keeps **Save** and
  **Run**; **Copy as curl**, **Copy to collection…** and **Delete file** moved
  into its **⋯** menu. Icons, menus with keyboard navigation, focus rings,
  status pills and a run progress bar replace the text glyphs.
- **Clearer environment picker.** The dropdown lists only real environments;
  the "no env" choice is gone (with a default set, it ran the default anyway),
  and the default is simply selected first. In **Manage environments…**,
  `.env.local` sits in its own **Local overrides** section, which applies on
  top of whichever environment is selected.

### Docs

- New **AI agents and MCP** page, [`llms.txt`](https://sankh.dev/llms.txt)
  for language models, and an agent skill in `skills/sankh/SKILL.md`.

## [0.4.0] - 2026-10-05

### Added

- **Manage environments in the UI.** A **Manage** button next to each
  collection's environment picker opens an editor for `environments/*.env`
  and `.env.local`: add, edit and remove variables (secret-looking values are
  masked until revealed), and create, copy, rename, delete or set the default
  environment. Saving keeps comments and `${VAR}` references as written.
- **Postman import creates `.env.local`.** Variables that requests use but
  neither the collection nor an imported environment defines are now listed
  in `.env.example` and the import report. Import also writes `.env.local`
  with sample values guessed from each name (`http://localhost:8080` for
  URLs, `1` for ids, `changeme` for tokens and the rest). An existing
  `.env.local` is never overwritten, even with `--force`.

## [0.3.0] - 2026-10-05

### Added

- **Import from Postman in the UI.** **Import Postman...** in `sankh serve`
  and the desktop app converts a Postman export (plus optional environments),
  shows the import report before writing, writes the new folder and adds it
  to the workspace. The desktop app picks the destination with the native
  folder dialog. Backed by `POST /api/import/postman`.
- **Homebrew cask for the desktop app**:
  `brew install --cask sankh-dev/tap/sankh-desktop`. The release workflow
  publishes it to the existing tap next to the CLI formula.

### Docs

- New **Assertions and chaining** guide: `@expect` examples (including
  arrays), `@capture` sources, and a worked multi-step flow based on the
  petstore example.

## [0.2.1] - 2026-10-03

### Fixed

- Desktop app (Linux AppImage): requests failed with `curl: symbol lookup
  error ... libnghttp2` because request files inherited the AppImage's
  `LD_LIBRARY_PATH`, which pointed the system `curl` at older bundled
  libraries. Request files now get the environment without the AppImage's
  paths.

## [0.2.0] - 2026-10-03

### Added

- **Desktop app** for macOS, Windows and Linux (Tauri 2). It is the web UI in
  a native window with a native folder picker. The server runs in-process on
  a random loopback port behind a per-launch token, and the app opens the
  saved workspace. Installers are attached to the GitHub release and are not
  code signed yet.
- **Workspaces**: `sankh serve` shows several collections side by side, each
  with its own environments, captured values, trust and runs.
  `sankh workspace add|list|remove` manages the saved workspace
  (`~/.config/sankh/workspace.toml`). `sankh serve A B` opens a session-only
  workspace.
- **Scratch**, a built-in, always-trusted collection for trying requests
  without a folder, plus **Copy to…** to move a request into a real
  collection.
- **`sankh import postman`** converts Postman v2.0/v2.1 collections and
  environments into a Sankh folder. Common test scripts become `@expect` and
  `@capture`, and secrets go to `.env.example` instead of being written to
  disk.
- UI: add-folder dialog with a directory browser, and per-collection
  sections in the sidebar.

### Changed

- The server API is now per collection, under `/api/c/{id}/...`.

## [0.1.1] - 2026-10-01

- Windows fixes for script paths and CLI tests.

## [0.1.0] - 2026-10-01

- First release: `sankh run`, `sankh serve`, `sankh init`, `sankh trust`.
