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
  ordering or trust, and collections cannot be nested inside each other.

## `sankh.toml`

All keys are optional.

```toml
name = "petstore"          # display name; default: folder name
format = 1                 # collection format version
default_env = "dev"        # environment used when --env is not given
timeout = 30               # per-request limit in seconds (curl --max-time)

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

The body is everything after the header. A file is in **form mode** when the
body is exactly one plain `curl` command (line continuations allowed) using
only quoting and `$VAR` / `${VAR}` expansion. Anything else (pipes, `;`,
redirection, command substitution, several commands) is **raw mode**: it is
edited as plain text and still runs, subject to trust.

### Interpreter

Files whose shebang names `bash` (or `zsh`) run with that shell; all others run
with `sh`. Files are sourced from the collection root as the working directory.

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

Env files use dotenv syntax. Missing variables referenced by a request produce
a warning.

## Variables and quoting

`$VAR` and `${VAR}` expand at run time. In the form view and the parsed model,
a literal dollar sign is written `\$` (for example, a `$` inside single quotes
in the file).

## Later annotations

`@expect header`, `@expect time`, `@expect body`, `@require`, `@secret`,
`@retry`, `@skip`, `@delay` and `@depends` are proposed for v1.x. They are recognised and ignored with a
warning by v1 binaries, so collections stay forward compatible.
