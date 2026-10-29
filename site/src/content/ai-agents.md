# AI agents and MCP

Sankh works with AI coding agents in two ways. Agents that can run shell
commands use the normal CLI and its JSON output. Any MCP client, including
chat apps without a shell, can use the built-in `sankh mcp` server.

## Agents with a shell

Cursor, Claude Code, Codex and similar agents can already drive Sankh:

```bash
sankh list my-api --json
sankh run my-api --env dev --report json --report-file -
```

`--report-file -` puts the JSON report on stdout and human output on stderr.
Exit codes are `0` passed, `1` an `@expect` or `@capture` failed, `2` a file
has an error (nothing was sent), `3` a request could not be sent. `sankh run
--check` validates files without sending anything, which suits an agent that
just wrote one.

To teach an agent the file format, point it at:

- [`llms.txt`](https://sankh.dev/llms.txt): a compact summary of the format,
  commands and rules, written for language models.
- The agent skill in the repository at
  [`skills/sankh/SKILL.md`](https://github.com/sankh-dev/sankh/blob/main/skills/sankh/SKILL.md).
  Copy the `skills/sankh` folder into `.cursor/skills/` or `.claude/skills/`
  in your project (or your home directory).

## MCP server

```
sankh mcp [PATH]...
```

Add it to your MCP client's configuration:

```json
{
  "mcpServers": {
    "sankh": { "command": "sankh", "args": ["mcp"] }
  }
}
```

Without a path it exposes your saved [workspace](/docs/workspaces) plus
Scratch. With paths (`"args": ["mcp", "/path/to/my-api"]`) it exposes only
those folders. It talks over stdin and stdout and opens no network port.

### Tools

| Tool | What it does |
| --- | --- |
| `list_collections` | Collection ids, names and folders |
| `list_requests` | The request tree of a collection, optionally filtered by tag |
| `show_request` | A request file's text and its parsed annotations |
| `list_environments` | Environment names, the default, and variables with secrets masked |
| `run` | Runs a request, folder or whole collection and returns the JSON report |

`run` takes `collection`, and optionally `path` (a file or folder inside the
collection), `env`, `folders`, `tags` and `bail`. Each call starts with fresh
captures; requests in one call chain as they do in `sankh run`. Response
bodies are truncated to keep results small.

### Safety

- The server is **read and run only**. It never creates or edits files,
  environments or the workspace.
- Request files never execute anything: Sankh sends the request itself (see
  [Secrets and safety](secrets-safety.md)).
- `run` checks every selected file first. If one has an error, it returns the
  errors with their line and column and sends nothing.
- Secrets are masked in every result exactly as in `sankh run`.
- `run` is marked as a tool with side effects, so clients ask before running
  requests.
