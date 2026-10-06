---
name: sankh
description: Write, run and debug Sankh API collections (folders of `.sh` files holding one curl command each, with `# @expect` / `# @capture` annotations). Use when the user works with a Sankh collection, a `sankh.toml`, `environments/*.env`, or asks to add, test or chain API requests with sankh.
---

# Sankh

Sankh runs a folder of curl files as an API collection. Request files are
shell scripts, so trust and secrets rules matter as much as the syntax.

## Before running anything

1. Find the collection root: the nearest folder with `sankh.toml` or
   `environments/`.
2. Check trust: `sankh workspace list --json` shows trust for saved
   collections; otherwise just run and look for exit code 3.
3. If it is not trusted, stop and ask the user to review the files and run
   `sankh trust <root>`. Never pass `--trust` or set `SANKH_TRUST=1` yourself;
   that is for CI checkouts only.

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
- Keep the body a single plain `curl` command (no pipes, `;`, `$(...)`) so the
  UI can edit it as a form. Use `$VAR` / `${VAR}` for every host, id and secret.
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
Exit codes: 0 passed, 1 a request failed, 2 usage/config error, 3 not trusted.

In the report, each result has `outcome` (`passed`, `failed`, `error`),
`assertions[]` with `label`, `passed`, `message`, the `response` (status,
headers, body), `captures`, `error` and `warnings`. A request that depends on a
captured value must run in the same `sankh run` as the request that captures
it, so run the folder (or collection), not the single file.

## Debugging checklist

- `error` mentions a missing variable: add it to the environment or `.env.local`.
- `warnings` about unknown annotations: check spelling against the table above.
- A malformed annotation stops the request; the error gives the line number.
- A non-JSON response fails every `@expect json`; check `response.body`.
