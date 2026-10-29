# Contributing

## Layout

| Path | What |
| --- | --- |
| `crates/sankh-core` | Parser (`parse/`: tokenizer, flag table, request builder), HTTP engine (`http/`), env layering, runner, assertions, captures, reports. All I/O the front ends share. |
| `crates/sankh-server` | `axum` API, auth and Origin/Host guards, SSE runs, embedded UI. |
| `crates/sankh-mcp` | `sankh mcp`: stdio MCP server (read and run only) on top of `sankh-core`. |
| `crates/sankh-cli` | The `sankh` binary. |
| `frontend/` | Svelte 5 + Vite + TypeScript UI, embedded into the server at build time. |
| `examples/petstore` | Reference collection; `cargo run --example petstore_mock` serves its API. |
| `docs/format.md` | The collection format spec (frozen for v1). |

## Checks

```bash
cargo fmt --all
cargo clippy --workspace --exclude sankh-desktop --all-targets -- -D warnings
cargo test
SANKH_CURL_PARITY=1 cargo test -p sankh-core --test parity   # compare with the real curl
SANKH_UPDATE_DOCS=1 cargo test -p sankh-core --test flags_doc  # after changing the flag table
(cd frontend && npm run check && npm run build)
cargo clippy -p sankh-desktop -- -D warnings   # needs webkit2gtk-4.1 on Linux
```

Snapshot tests use `insta`; review changes with `cargo insta review`.

## Ground rules

- The annotation set is frozen. Propose new annotations in the annotation
  reference first, with a concrete need.
- Request files must stay runnable without Sankh. Nothing may rewrite the curl
  line at run time.
- Never print a secret: route all user-visible output through `Redactor`.
- Never execute anything from a request file: no shell, no `curl`, no
  process. Request files are data.
- Supporting a curl flag means an entry in `parse/flags.rs` with its tier, a
  test in `tests/wire.rs` (and `tests/parity.rs` when curl can be compared),
  and the regenerated `site/src/content/curl-flags.md`. A flag Sankh cannot
  honor is an error, never silently ignored.
- Files are read only from inside the collection root.
