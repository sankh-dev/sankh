# Import from Postman

```
sankh import postman COLLECTION.json [--env ENV.json]... [-o DIR] [--force] [--json]
```

Converts a Postman Collection v2.0 or v2.1 export into a Sankh folder. To
bring a single request into the UI instead, paste its `curl` command into
**New request**.

```bash
sankh import postman "My API.postman_collection.json" \
  --env dev.postman_environment.json --env prod.postman_environment.json \
  -o my-api
sankh trust my-api
sankh run my-api --env dev
```

## What gets converted

| Postman | Sankh |
| --- | --- |
| Folders | Directories |
| Requests | Numbered `.sh` files, one `curl` command each |
| Collection and environment variables | `environments/*.env` |
| `{{baseUrl}}` | `${BASE_URL}` |
| Auth (bearer, basic, API key) | Explicit headers, `-u`, or a query parameter |
| Common test-script statements | `@expect` and `@capture` |

Test-script statements that convert include:

- `pm.response.to.have.status(201)` becomes `@expect status 201`.
- `pm.environment.set("token", pm.response.json().token)` becomes
  `@capture TOKEN=.token`.
- Simple `pm.expect(...).to.eql(...)` checks become `@expect json ...`.

Sankh does not run JavaScript. Anything else in a script is kept as comments in
the request file and listed in the import report, so you can see what still
needs a manual look.

## Secrets

Secret values and literal credentials are never written to disk. Their names
go into `.env.example`; set the values in `.env.local`, which the generated
`.gitignore` excludes.

## Safety

Import never writes into a non-empty folder unless you pass `--force`. Use
`--json` to get the import report in machine-readable form.
