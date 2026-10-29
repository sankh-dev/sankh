# Sankh collection format, v1

Status: **frozen** for v1 (`format = 1` in `sankh.toml`). New annotations are
added only through the annotation reference, one at a time.

## Folder layout

```
my-api/
  sankh.toml                 # optional
  .env.example
  .gitignore                 # includes .env.local
  environments/
    dev.env
    prod.env
  auth/
    01-login.sh
  users/
    01-list.sh
    02-create.sh
```

- The collection root is the nearest folder containing `sankh.toml` or
  `environments/`. Without either, the folder passed to Sankh is the root.
- Request files end in `.sh`. Hidden entries and the root-level
  `environments/`, `node_modules/` and `target/` folders are ignored.
- Folders with no request files are not shown.
- A collection is self-contained. When several collections are open in one
  workspace (`sankh serve`), they never share environments, captured values,
  or ordering, and collections cannot be nested inside each other.

## `sankh.toml`

All keys are optional.

```toml
name = "petstore"          # display name; default: folder name
format = 1                 # collection format version
default_env = "dev"        # environment used when --env is not given
timeout = 30               # per-request limit in seconds (curl --max-time); default 30
max_body = "10MB"          # largest response body kept; bytes or "512KB", "10MB", "1GB"
env_proxy = true           # honor HTTP_PROXY, HTTPS_PROXY and NO_PROXY

[order]                    # explicit ordering per folder ("" is the root)
"" = ["auth", "users"]
"users" = ["02-create.sh", "01-list.sh"]
```

## Ordering

Entries with a numeric prefix (`01-`, `2_`, `10.`) come first, sorted by the
number; the rest follow alphabetically (case-insensitive). Entries listed in
`[order]` for a folder come before all others, in the listed order.

## Request file

```bash
#!/usr/bin/env bash
# @name Create user
# @tags smoke
# @capture USER_ID=.data.id
# @expect status 201
# @expect json .data.name == "test"
curl -sS -X POST "$BASE_URL/users" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"test"}'
```

### Header block

- The header is the contiguous run of `#` lines at the top of the file. A
  shebang may come first. A blank line or any non-comment line ends it.
- An annotation is a header line of the form `# @<name> <arguments>`; the space
  after `#` is optional. One annotation per line.
- Other comment lines are preserved verbatim.
- Annotations after the header are ignored with a warning.
- **Unknown annotations** produce a warning. **Malformed known annotations**
  are errors reported with the line number; the request does not run.

### Body

The body is everything after the header and must be exactly one `curl`
command. Line continuations (`\` at the end of a line), single and double
quotes, backslash escapes, `$'...'` strings and blank or comment lines around
the command are allowed.

Sankh does not run the file. It reads the command and sends the request
itself, so:

- Shell syntax is an error: pipes, `;`, `&&`, `||`, redirections, `$(...)`,
  backticks, `$((...))`, assignments, and a second command. The file is still
  shown in the editor as text (**raw mode**), but it does not run.
- Every flag must be one Sankh supports or knowingly ignores; anything else
  is an error that names the flag. See [curl flags](https://sankh.dev/docs/curl-flags).
- Globs (`*`, `?`, `[...]`) and `~` are literal.
- File arguments (`-d @body.json`, `-F f=@logo.png`, `--cacert ca.pem`,
  `-b`/`-c` cookie files) are relative to the collection root and must stay
  inside it.
- The shebang is kept but has no effect.

Every problem is reported before anything is sent, with the file, line and
column:

```text
error: flag --tlsv1.3 is not supported yet
 --> auth/01-login.sh:2:28
  |
2 | curl -sS "$BASE_URL/login" --tlsv1.3 \
  |                            ^^^^^^^^^
  = help: supported flags: https://sankh.dev/docs/curl-flags
```

### Limits

| Limit | Default | Change it with |
| --- | --- | --- |
| Each attempt, redirects included | 30 s | `-m`, `# @timeout`, `timeout` |
| Response body kept | 10 MB | `max_body` |
| Redirects followed with `-L` | 50 | `--max-redirs` |

A body over `max_body` is cut; `@expect json` and jq captures on it fail with
a message that says so.

## Annotations (v1)

| Annotation | Repeatable | Meaning |
| --- | --- | --- |
| `@name <text>` | no | Display name. Default: file name without number prefix and extension (`02-create-order.sh` → "create order"). |
| `@description <text>` | yes (one line each) | Notes shown in the UI. Several lines form a multi-line description; an empty `# @description` is a blank line. No effect on execution. |
| `@tags <tag>...` | yes (merged) | Labels for `--tag`. Lowercase letters, digits, `-`, `_`. |
| `@timeout <duration>` | no | Limit for this request (curl `--max-time`), overriding `timeout` in `sankh.toml`. `500ms`, `10s`, `2m`, or a bare number of seconds. |
| `@expect status <codes>` | yes (all must pass) | `200`, `2xx`, or `200\|201\|204`. |
| `@expect json <jq> <op> <value>` | yes (all evaluated) | Assertion on the JSON body. |
| `@expect json <jq> exists` | yes | The expression yields a non-null value. |
| `@capture NAME=<source>` | yes | Store a response value for later requests in the run. |

### `@expect status`

Without any `@expect status`, a request passes on any `2xx` status. Write an
explicit expectation for tests that expect errors, e.g. `@expect status 404`.

### `@expect json`

| Operator | Meaning |
| --- | --- |
| `==`, `!=` | JSON value equality (numbers compare numerically) |
| `>`, `>=`, `<`, `<=` | Numeric comparison; the value must be a number |
| `exists` | Non-null result |
| `matches "<regex>"` | String matches the regex |
| `contains <value>` | String contains substring, or array contains element |

- The value is a JSON literal: `"text"`, `42`, `true`, `null`, arrays or
  objects. Bare words are an error (`did you mean "word"?`).
- The line is split from the right: the value is the trailing JSON literal and
  the operator is the token just before it, so the jq expression may itself
  contain operators and pipes (`.items | length > 0`).
- Strings may embed variables: `@expect json .owner == "$USER_ID"`.
- The expression must produce exactly one value (except for `exists`).
- A non-JSON response fails the assertion with a clear message.

### `@capture`

| Source | Meaning |
| --- | --- |
| `<jq expression>` (starts with `.`) | Evaluated against the JSON body |
| `header <Name>` | Response header (case-insensitive) |
| `status` | HTTP status code |

- `NAME` matches `[A-Z_][A-Z0-9_]*`.
- `null`, no value or several values fail the request.
- Strings are stored unquoted; numbers and booleans as text; objects and
  arrays as compact JSON.
- Captures run only when every assertion passed, live only for the current run,
  and are never written to disk. Later captures override earlier ones.

See [Assertions and chaining](https://sankh.dev/docs/assertions) for worked examples, including
arrays and a multi-step flow.

## Environments

Lowest to highest priority:

1. `environments/<name>.env`
2. `.env.local` (gitignored)
3. The process environment (CI secrets)
4. Values captured earlier in the run

Env files use dotenv syntax. A variable that a request uses but nothing
defines is an error, reported before anything is sent. A variable that an
earlier request in the same run captures counts as defined.

## Variables and quoting

`$VAR`, `${VAR}` and `${VAR:-default}` expand at run time, inside double
quotes or unquoted; inside single quotes `$` is literal. A value is inserted
as is: it is never split on spaces, globbed or read again, so it cannot add
flags or quotes. Other `${...}` forms are errors.

In the form view and the parsed model, a literal dollar sign is written `\$`
(for example, a `$` inside single quotes in the file).

## Later annotations

`@expect header`, `@expect time`, `@expect body`, `@require`, `@secret`,
`@retry`, `@skip`, `@delay` and `@depends` are proposed for v1.x. They are recognised and ignored with a
warning by v1 binaries, so collections stay forward compatible.
