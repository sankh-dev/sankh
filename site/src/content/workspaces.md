# Workspaces

A workspace is the set of collections the UI shows side by side, so you can
work on a users API and a payments API at the same time without switching.
Collections stay independent: each has its own environment picker, captured
values and runs.

## The saved workspace

```bash
sankh workspace add ~/work/users-api ~/work/payments-api
sankh workspace list            # --json for scripts
sankh workspace remove users-api   # by id or path (alias: unlink)
sankh serve                     # opens the saved workspace
```

The saved workspace lives in `~/.config/sankh/workspace.toml` (or
`$SANKH_CONFIG_DIR/workspace.toml`). `sankh serve` with no path and the
[desktop app](desktop.md) both open it. Folders you add or unlink in the UI
are saved there too.

Removing a folder only unlinks it; the files stay on disk.

## Session-only workspaces

```bash
sankh serve ~/work/users-api ~/work/payments-api
```

With one or more paths, `sankh serve` opens just those folders for this
session. Folders added or unlinked in the UI are not saved, and the UI says
so.

## Scratch

**Scratch** is a built-in collection that is always listed first, for trying
requests without an existing folder:

- It lives in `~/.local/share/sankh/scratch` (or `$SANKH_DATA_DIR/scratch`).
- It cannot be unlinked.

When a request is worth keeping, use **Copy to collection…** in the editor's **⋯** menu to copy it
into a real collection. Existing files are never overwritten.

## Rules

- A missing folder stays in the list, marked missing, until you unlink it, so
  an unmounted drive does not silently drop it from your workspace.
- Collections cannot be nested inside each other.

## A folder of collections

Opening a folder that is not itself a collection, but whose child folders are
(`sankh.toml` or `environments/` in each one), adds each child on its own.
Each keeps its own environments. `sankh run` on that parent folder stops and
names the collections to run instead.

`sankh serve` with no path, and the desktop app, watch `~/sankh-collections`
while they are open. A collection folder dropped there is added. A folder you
have unlinked stays unlinked until you press **Refresh** on the Collections
sidebar. Refresh also reloads files and environments for collections that are
already open. If `~/sankh-collections` itself was opened as one collection,
Refresh replaces that entry with the child collections.
