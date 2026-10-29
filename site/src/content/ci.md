# Running in CI

## `sankh run`

```
sankh run [PATH] [--env NAME] [--folder F]... [--tag T]... [--all]
          [--check] [--report junit|json] [--report-file PATH|-] [--bail] [-v]
```

- `PATH` is a request file or folder (default `.`). Folders run in order:
  numeric prefix (`01-`), then name; override with `[order]` in `sankh.toml`.
- `--folder` and `--tag` are repeatable; a request matches if it is in **any**
  listed folder and has **any** listed tag.
- Before sending anything, every selected file is checked: flags, quoting,
  variables and `@file` paths. If any file has an error, all errors are
  printed with their file, line and column, and nothing is sent.
  `--check` stops after this step, which makes it a fast lint job.
- `--report` writes `sankh-report.xml` or `sankh-report.json` unless
  `--report-file` names another path; `--report-file -` writes the report to
  stdout and human output to stderr.
- `--bail` stops at the first failure; `-v` prints response bodies for passing
  requests too.

Values captured with `@capture` are available to every later request in the
same run, so a login request can feed its token to the rest of the collection
(see [Assertions and chaining](assertions.md)).

## Exit codes

| Exit code | Meaning |
| --- | --- |
| 0 | All requests passed |
| 1 | An `@expect` or `@capture` failed |
| 2 | A file could not be read or built (unsupported flag, shell syntax, undefined variable, missing `@file`, bad URL), or a usage error. Nothing was sent. |
| 3 | A request could not be sent: DNS, connection, TLS or timeout |

When a run has several kinds of failure, 2 wins over 3, and 3 over 1. Each
result in the JSON report has an `error_kind` (`assertion`, `parse`,
`config` or `transport`).

## GitHub Actions

```yaml
- run: curl -fsSL https://sankh.dev/install.sh | sh
- run: sankh run . --env ci --folder smoke --report junit
  env:
    TOKEN: ${{ secrets.API_TOKEN }}
```

If the install directory is not already on the `PATH`, the installer appends it
to `GITHUB_PATH`, so `sankh` is available in later steps.

## GitLab CI

```yaml
api-smoke:
  image: alpine:latest
  before_script:
    - apk add --no-cache curl
    - curl -fsSL https://sankh.dev/install.sh | sh
    - export PATH="$HOME/.local/bin:$PATH"
  script:
    - sankh run . --env ci --tag smoke --report junit --report-file report.xml
  artifacts:
    reports:
      junit: report.xml
```

## Reports

`--report junit` produces a JUnit XML file that most CI systems render as a test
report. `--report json` produces a machine-readable summary with per-request
results, assertions and captures. Secrets are redacted in both.

```bash
sankh run . --env ci --report json --report-file - | jq '.summary'
```

## Environments in CI

Environment layers, lowest to highest priority: `environments/<name>.env`,
`.env.local`, the process environment, then values captured earlier in the run.
In CI, keep `environments/ci.env` for non-secret values and pass secrets as
process environment variables from your CI secret store.
