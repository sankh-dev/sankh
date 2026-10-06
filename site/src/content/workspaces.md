# Workspaces

A workspace is the set of collections the UI shows side by side, so you can
work on a users API and a payments API at the same time without switching.
Collections stay independent: each has its own environment picker, captured
values, trust and runs.

## The saved workspace

```bash
sankh workspace add ~/work/users-api ~/work/payments-api
sankh workspace list            # with trust status; --json for scripts
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
- It is trusted automatically, because only you write to it.
- It cannot be unlinked.

When a request is worth keeping, use **Copy to collection…** in the editor's **⋯** menu to copy it
into a real collection. Existing files are never overwritten.

## Rules

- A missing folder stays in the list, marked missing, until you unlink it, so
  an unmounted drive does not silently drop it from your workspace.
- Collections cannot be nested inside each other.
- Each collection is trusted on its own; see
  [Trust and secrets](trust-secrets.md).
