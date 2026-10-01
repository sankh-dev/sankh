# Contributing

## Layout

| Path | What |
| --- | --- |
| `crates/sankh-core` | Parser, env layering, runner, assertions, captures, trust, reports. All I/O both front ends share. |
| `crates/sankh-server` | `axum` API, auth and Origin/Host guards, SSE runs, embedded UI. |
| `crates/sankh-cli` | The `sankh` binary. |
| `frontend/` | Svelte 5 + Vite + TypeScript UI, embedded into the server at build time. |
| `examples/petstore` | Reference collection; `cargo run --example petstore_mock` serves its API. |
| `docs/format.md` | The collection format spec (frozen for v1). |

## Checks

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd frontend && npm run check && npm run build)
```

Snapshot tests use `insta`; review changes with `cargo insta review`.

## Ground rules

- The annotation set is frozen. Propose new annotations in the annotation
  reference first, with a concrete need.
- Request files must stay runnable without Sankh. Nothing may rewrite the curl
  line at run time.
- Never print a secret: route all user-visible output through `Redactor`.
- An untrusted collection must never execute. Execution requires a `Trusted`
  value, which only `trust::ensure` produces.
