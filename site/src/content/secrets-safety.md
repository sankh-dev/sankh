# Secrets and safety

## Request files never execute

A request file holds one curl command, but Sankh does not run it. It reads the
command, checks every flag against the [supported curl flags](curl-flags.md),
expands variables, and sends the request with its own HTTP client. There is no
shell and no `curl` process, so opening or running a collection someone else
wrote cannot execute anything on your machine:

- Pipes, `;`, `&&`, redirections, `$(...)` and backticks are errors, reported
  before anything is sent.
- A variable's value is inserted as is. It is never split, globbed or read
  again, so a value cannot add flags or quotes.
- File arguments (`-d @body.json`, `-F file=@logo.png`, `--cacert`, cookie
  files) are relative to the collection root and cannot leave it.

What a request file can still do is send a request: to any host, with any
values from your environment. Review a collection's URLs before running it
with real secrets.

## Secrets

- Values of variables whose names contain `TOKEN`, `SECRET`, `KEY`,
  `PASSWORD`, `PASSWD`, `AUTH` or `COOKIE` (and are at least 4 characters
  long) are replaced with `***` in `sankh run`
  output, JUnit and JSON reports, and `sankh mcp` results, including captured
  values. The request as sent is shown with `Authorization`,
  `Proxy-Authorization` and `Cookie` headers masked too.
- The web UI and the desktop app show real values in run results, so you can
  inspect a token you just captured. Environment editors still hide secret
  values behind an eye toggle until you reveal them.
- Keep secrets in `.env.local` (gitignored) or CI secrets, never in
  `environments/*.env`.
- Values stay inside the Sankh process. They never appear on a command line,
  so they are not visible in `ps`.

## TLS

Certificates are verified against the system's trusted roots. For a private
CA, add `--cacert ca.pem` to the request. `-k` turns verification off; Sankh
then prints a warning once per run, and the UI shows a banner on the response.

## Sending requests from a shared server

`sankh serve` sends requests from the machine it runs on. Anyone who can use
the UI can make that machine send requests, including to hosts on its private
network, such as cloud metadata endpoints. When others can reach the server,
limit where requests may go:

```bash
sankh serve --listen 0.0.0.0 --token "$SANKH_TOKEN" \
  --allow-target '*.api.example.com' --deny-target 169.254.169.254
```

`--allow-target` and `--deny-target` take host names, `*.suffix` patterns, IP
addresses and CIDR ranges, and can be repeated. With any `--allow-target`,
only matching hosts are allowed; `--deny-target` wins over it. Each redirect
is checked too.

## Telemetry

None. Sankh makes no network requests other than the ones in your files.
