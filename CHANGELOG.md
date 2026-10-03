# Changelog

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
