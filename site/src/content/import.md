# Import from Postman

```
sankh import postman COLLECTION.json [--env ENV.json]... [-o DIR] [--force] [--json]
```

Converts a Postman Collection v2.0 or v2.1 export into a Sankh folder. To
bring a single request into the UI instead, choose **+** (new request) on a
collection or folder and paste its `curl` command.

```bash
sankh import postman "My API.postman_collection.json" \
  --env dev.postman_environment.json --env prod.postman_environment.json \
  -o my-api
sankh trust my-api
sankh run my-api --env dev
```

## From the app

The [web UI](serve.md) and the [desktop app](desktop.md) do the same
conversion without a terminal:

1. Choose **Add collection > Import from Postman…** in the header.
2. Pick the exported collection and, optionally, one or more environment
   exports. Sankh shows the import report before writing anything: request
   and folder counts, renamed variables, the values to set in `.env.local`,
   and warnings for each file.
3. Check the destination folder. It defaults to a new folder named after the
   collection in your home folder; the desktop app also has a **Choose...**
   button. If the folder is not empty, tick the overwrite box to write into
   it anyway.
4. Choose **Import**. The new collection is added to the workspace and
   selected. Review the files, then trust it to run requests.

The destination cannot be inside, or contain, a collection that is already
open.

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

Variables that requests use but neither the collection nor an imported
environment defines are listed the same way, so nothing goes missing silently
when you import without environment files.

Import also creates `.env.local` with a sample value for each of these
variables, guessed from its name: `http://localhost:8080` for `*_URL` and
`*_HOST`, `1` for `*_ID`, `user@example.com` for `*_EMAIL`, and `changeme` for
tokens, keys, passwords and anything else. Replace them before real use, in the
file or from **Manage environments…** in the collection's **⋯** menu in the UI.

## Safety

Import never writes into a non-empty folder unless you pass `--force`. An
existing `.env.local` is never overwritten, even with `--force`. Use
`--json` to get the import report in machine-readable form.
