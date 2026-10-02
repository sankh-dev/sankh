# Trust and secrets

## Trust

Request files are shell scripts, so a cloned collection never runs until you
trust it:

```bash
sankh trust my-api
```

Trust is stored in `~/.config/sankh/trust.toml` (or `$SANKH_CONFIG_DIR`). In a
git repository the HEAD commit is recorded, and trust lapses when HEAD changes,
so pulled changes are reviewed before they run.

Running an untrusted collection exits with code `3`. In CI, where the checkout
is the code under review, pass `--trust` or set `SANKH_TRUST=1`.

## Secrets

- Values of variables whose names contain `TOKEN`, `SECRET`, `KEY`,
  `PASSWORD`, `AUTH` or `COOKIE` are replaced with `***` in all output,
  reports and UI responses, including captured values.
- Keep secrets in `.env.local` (gitignored) or CI secrets, never in
  `environments/*.env`.

## Known limitation

The shell expands variables into curl's arguments, so a secret can appear in
the process list (`ps`) while a request runs. Use curl's `-H @file` or
`--config` in a request file if that matters.

## Telemetry

None. Sankh makes no network requests other than the ones in your files.
