# Trust and secrets

## Trust

Request files are shell scripts, so a cloned collection never runs until you
trust it:

```bash
sankh trust my-api
```

Trust is stored in `~/.config/sankh/trust.toml` (or `$SANKH_CONFIG_DIR`). In a
git repository the HEAD commit is recorded. When HEAD moves (a `git pull`, or
checking out a teammate's branch), trust carries forward if no file under the
trusted folder differs from the trusted commit. If any file does, trust lapses
and the changed files are listed so you can review them before anything runs.
If git can't compute the diff, trust lapses too.

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
