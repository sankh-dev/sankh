# Web UI

## `sankh serve`

```
sankh serve [PATH] [--listen 127.0.0.1] [--port 4747] [--token T] [--allow-host H]...
```

The UI has a file tree, form and raw (CodeMirror) editors, an environment
picker, live results for single requests and whole folders, and import from
curl. The server, not the browser, executes the requests, so there are no CORS
workarounds and the results match `sankh run` exactly.

Values captured in the UI (for example a login token) are kept per environment
for the session, so you can run requests one at a time. **Clear captures**
forgets them.

Edits made in the UI are written back to the same `.sh` files, so a change in
the browser is a normal diff in git.

## Security model

- Binds to `127.0.0.1` by default and rejects foreign `Host` headers, which
  blocks DNS rebinding.
- Any other `--listen` address requires `--token` (or `SANKH_TOKEN`). The
  printed URL carries the token in the fragment (`#token=...`), which never
  reaches server logs.
- Cross-origin API calls are rejected; paths cannot leave the collection root.

## Remote use

Prefer an SSH tunnel, which keeps the server bound to localhost:

```bash
ssh -L 4747:localhost:4747 host
# then, on the remote host:
sankh serve my-api
```

Or put it behind a TLS reverse proxy and allow the public host name:

```bash
sankh serve my-api --listen 0.0.0.0 --token "$SANKH_TOKEN" \
  --allow-host api-tools.example.com
```
