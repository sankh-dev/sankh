---
name: sankh
description: Write, run and debug Sankh API collections (folders of `.sh` files holding one curl command each, with `# @expect` / `# @capture` annotations). Use when the user works with a Sankh collection, a `sankh.toml`, `environments/*.env`, or asks to add, test or chain API requests with sankh.
---

# Sankh

Sankh runs a folder of curl files as an API collection. It reads each curl
command and sends the request with its own HTTP client; it never runs a shell
or `curl`, so a file can only describe a request, never execute anything.

## Before running anything

1. Find the collection root: the nearest folder with `sankh.toml` or
   `environments/`.
2. Run `sankh run <root> --check --env <env>`. It parses every file and checks
   variables and `@file` paths without sending anything, and reports every
   problem with its file, line and column.

If the `sankh` MCP server is available, prefer its tools (`list_collections`,
`list_requests`, `show_request`, `list_environments`, `run`) over the shell.

## Writing a request file

Name it with a numeric prefix for ordering, e.g. `users/02-create.sh`:

```bash
#!/usr/bin/env bash
# @name Create user
# @tags smoke
# @expect status 201
# @expect json .data.name == "test"
# @capture USER_ID=.data.id
curl -sS -X POST "$BASE_URL/users" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"test"}'
```

- Annotations go in the contiguous `#` block at the top; a blank line ends it.
- The body must be exactly one `curl` command. Pipes, `;`, `&&`, redirections,
  `$(...)` and a second command are errors, and so is any curl flag Sankh does
  not support (`-o`, `-w`, `-D`, `--tlsv1.2`, ...); use `@expect` and
  `@capture` instead of `-w` or `| jq`. Supported flags:
  https://sankh.dev/docs/curl-flags
- Use `$VAR`, `${VAR}` or `${VAR:-default}` for every host, id and secret. An
  undefined variable is an error. Values are never split or re-read.
- File arguments (`-d @body.json`, `-F file=@logo.png`) are relative to the
  collection root and cannot leave it.
- For JSON bodies use `--json '{...}'`, or `-d` with
  `-H 'Content-Type: application/json'`; plain `-d` sends
  `application/x-www-form-urlencoded`, as curl does.
- `@expect status`: `200`, `2xx`, `200|201|204`. Without it, any 2xx passes;
  add an explicit one for error tests (`@expect status 404`).
- `@expect json <jq> <op> <value>`: ops `==` `!=` `>` `>=` `<` `<=`
  `contains` `matches "<regex>"`, or `<jq> exists`. The value must be a JSON
  literal (`"text"`, `42`, `true`, `null`); bare words are an error.
- `@capture NAME=<source>`: a jq expression (`.id`), `header <Name>`, or
  `status`. `NAME` is `[A-Z_][A-Z0-9_]*`. Later requests in the same run use it
  as `$NAME`. Captures only run when every assertion passed.

Full reference: `docs/format.md` in the Sankh repo or https://sankh.dev/docs/format.

## Variables and secrets

- Layers, lowest to highest: `environments/<name>.env`, `.env.local`, process
  environment, captured values.
- Non-secret values (`BASE_URL`) go in `environments/<name>.env`. Secrets go in
  `.env.local` (gitignored); list their names in `.env.example`.
- Names containing TOKEN, SECRET, KEY, PASSWORD, PASSWD, AUTH or COOKIE are
  masked as `***` in all output.

## Running and reading results

```bash
sankh list <root> --json
sankh run <root> --env dev --report json --report-file -       # whole collection
sankh run <root>/users/02-create.sh --env dev --report json --report-file -
sankh run <root> --tag smoke --folder users --bail
```

With `--report-file -` the JSON report is on stdout and human output on stderr.
Exit codes: 0 passed; 1 an `@expect` or `@capture` failed; 2 a file could not
be read or built (unsupported flag, shell syntax, undefined variable, missing
file), or a usage error, and nothing was sent; 3 a request could not be sent
(DNS, connection, TLS, timeout).

In the report, each result has `outcome` (`passed`, `failed`, `error`),
`error_kind` (`assertion`, `parse`, `config`, `transport`), `assertions[]` with
`label`, `passed`, `message`, the `request` as sent (method, url, headers),
the `response` (status, headers, body, redirects), `captures`, `error`,
`warnings` and `diagnostics` (with `line` and `column`). A request that depends on a
captured value must run in the same `sankh run` as the request that captures
it, so run the folder (or collection), not the single file.

## Debugging checklist

- `error` mentions a missing variable: add it to the environment or `.env.local`,
  or run the request together with the one that captures it.
- `flag ... is not supported`: rewrite with a supported flag (the error's help
  line usually says how), or drop it.
- `error_kind: transport`: the server was not reached; check `BASE_URL`, the
  network, or TLS (`--cacert` for a private CA).
- `warnings` about unknown annotations: check spelling against the table above.
- A malformed annotation stops the request; the error gives the line number.
- A non-JSON response fails every `@expect json`; check `response.body`.
