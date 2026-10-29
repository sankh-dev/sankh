<!-- Generated from crates/sankh-core/src/parse/flags.rs; run `SANKH_UPDATE_DOCS=1 cargo test -p sankh-core --test flags_doc` to refresh. -->

# curl flags

Sankh reads request files as curl commands and sends them with its own HTTP client. It never runs `curl` or a shell. A flag that is not listed as supported is an error, never silently ignored, so a request cannot behave differently from what the file says.

Short switches can be combined (`-sSL`), and a value can follow a short flag directly (`-XPOST`). `--flag=value` is not curl syntax; write `--flag value`.

## Supported

| Flag | Behavior |
| --- | --- |
| `--url` `<url>` | The URL, as an alternative to a positional argument. Exactly one URL per file; `http` or `https`. Without a scheme, `http://` is assumed. |
| `-X`, `--request` `<method>` | Request method. |
| `-H`, `--header` `<header>` | Adds a header; duplicates are sent in order. A header of the same name replaces the one Sankh would add. `Name:` removes it; `Name;` sends it with an empty value. |
| `-d`, `--data` `<data>` | Request body; makes the request a `POST`. Several data flags are joined with `&`. `@file` reads a file and strips CR and LF. Without a `Content-Type` header, `application/x-www-form-urlencoded` is sent, as curl does. |
| `--data-ascii` `<data>` | Same as `-d`. |
| `--data-raw` `<data>` | Like `-d`, but `@` has no special meaning. |
| `-u`, `--user` `<user:password>` | Basic authentication. Without `:password` the password is empty (no prompt). |
| `-G`, `--get` | Sends the data flags as the query string of a `GET`. |
| `-L`, `--location` | Follows redirects. A `POST` becomes a `GET` without a body on 301, 302 and 303; 307 and 308 keep the method and body. `Authorization` and `Cookie` headers are not sent to another host. |
| `--location-trusted` | Like `-L`, and keeps sending credentials to other hosts. |
| `--max-redirs` `<num>` | Most redirects to follow with `-L` (default 50; -1 for no limit). |
| `-m`, `--max-time` `<seconds>` | Limit for each attempt, redirects included; decimals allowed. Overrides `# @timeout` and `timeout` in `sankh.toml`; 0 keeps them. |
| `--connect-timeout` `<seconds>` | Limit for connecting, decimals allowed; 0 for none. |
| `--compressed` | Sends `Accept-Encoding` and decodes gzip, deflate and brotli bodies. Without it the body is returned as received. |
| `-k`, `--insecure` | Skips certificate and hostname verification. Sankh prints a warning once per run. |
| `--data-urlencode` `<data>` | URL-encodes data: `name=value` encodes the value, `=value` the whole value, `name@file` and `@file` a file's contents. |
| `--data-binary` `<data>` | Like `-d`, but `@file` keeps the file's bytes as they are. |
| `-A`, `--user-agent` `<name>` | Sets `User-Agent` (default `sankh/<version>`). An empty value removes it. |
| `-e`, `--referer` `<url>` | Sets `Referer`. |
| `-I`, `--head` | Sends a `HEAD` request. |
| `--json` `<data>` | JSON body: like `--data-binary`, and sets `Content-Type` and `Accept` to `application/json`. Several are concatenated. |
| `-F`, `--form` `<name=content>` | Multipart form field: `name=value`, `name=@file` (upload), `name=<file` (text from a file), with `;type=` and `;filename=`. |
| `--form-string` `<name=string>` | Multipart field whose value is used as is (`@`, `<` and `;` are literal). |
| `-b`, `--cookie` `<data\|file>` | `name=value` pairs to send, or a cookie file to load (Netscape format). Enables the run's cookie jar. |
| `-c`, `--cookie-jar` `<file>` | Writes the run's cookies to a file in the collection, and keeps cookies between requests of the run. |
| `-x`, `--proxy` `<url>` | HTTP or HTTPS proxy. Without it, `HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY` apply (set `env_proxy = false` in `sankh.toml` to ignore them). |
| `--noproxy` `<hosts>` | Comma-separated hosts that bypass the proxy; `*` for all. |
| `--cacert` `<file>` | PEM file of certificates to trust instead of the system store. |
| `-E`, `--cert` `<file>` | Client certificate (PEM), optionally with its private key. |
| `--key` `<file>` | Private key (PEM) for `--cert`. |
| `--resolve` `<host:port:addr>` | Connects to `addr` for `host:port` instead of resolving the name. |
| `--retry` `<num>` | Retries transient failures (timeouts and 408, 429, 500, 502, 503, 504), waiting 1 s and doubling, or as `Retry-After` says. |
| `--retry-delay` `<seconds>` | Fixed wait between `--retry` attempts; 0 keeps the doubling wait. |
| `--http1.1` | Uses HTTP/1.1 only. |
| `--http2` | Uses HTTP/2 when the server offers it over TLS (the default). |

## Accepted with no effect

| Flag | Behavior |
| --- | --- |
| `-s`, `--silent` | Accepted; no effect. |
| `-S`, `--show-error` | Accepted; no effect. |
| `-i`, `--include` | Accepted; no effect. Sankh always records response headers. |
| `-v`, `--verbose` | Accepted; no effect. Sankh always records the request it sent. |
| `-g`, `--globoff` | Accepted; no effect. Sankh never expands `[]` and `{}` in URLs. |
| `-f`, `--fail` | Accepted; no effect. Use `# @expect status` to check the status. |
| `--fail-with-body` | Accepted; no effect. Use `# @expect status` to check the status. |
| `-N`, `--no-buffer` | Accepted; no effect. |
| `--no-progress-meter` | Accepted; no effect. |
| `-#`, `--progress-bar` | Accepted; no effect. |
| `--basic` | Accepted; no effect. Basic is the only authentication scheme. |

## Not allowed

| Flag | Behavior |
| --- | --- |
| `-o`, `--output` `<file>` | Sankh records the status, headers, body and timing itself; use `# @capture` and `# @expect`. |
| `-O`, `--remote-name` | Sankh records the status, headers, body and timing itself; use `# @capture` and `# @expect`. |
| `-D`, `--dump-header` `<file>` | Sankh records the status, headers, body and timing itself; use `# @capture` and `# @expect`. |
| `-w`, `--write-out` `<format>` | Sankh records the status, headers, body and timing itself; use `# @capture` and `# @expect`. |
| `--output-dir` `<dir>` | Sankh records the status, headers, body and timing itself; use `# @capture` and `# @expect`. |
| `-J`, `--remote-header-name` | Sankh records the status, headers, body and timing itself; use `# @capture` and `# @expect`. |

## Not supported yet

| Flag | Behavior |
| --- | --- |
| `--http3` | HTTP/3 is not supported. |
| `--http3-only` | HTTP/3 is not supported. |
| `-0`, `--http1.0` | HTTP/1.0 is not supported. |
| `--http2-prior-knowledge` | Use `--http2` over HTTPS. |
| `-T`, `--upload-file` `<file>` | Use `-X PUT --data-binary @file`. |
| `-K`, `--config` `<file>` | Write the options in the request file. |
| `-:`, `--next` | Put each request in its own file. |
| `--oauth2-bearer` `<token>` | Use `-H "Authorization: Bearer $TOKEN"`. |
| `--aws-sigv4` `<provider>` | Only Basic authentication (`-u`) is supported. |
| `--digest` | Only Basic authentication (`-u`) is supported. |
| `--ntlm` | Only Basic authentication (`-u`) is supported. |
| `--negotiate` | Only Basic authentication (`-u`) is supported. |
| `--anyauth` | Only Basic authentication (`-u`) is supported. |
| `-r`, `--range` `<range>` | Use `-H "Range: bytes=0-99"`. |
| `--path-as-is` | Dot segments in URLs are always normalized. |
| `--request-target` `<path>` |  |
| `--unix-socket` `<path>` |  |
| `--abstract-unix-socket` `<path>` |  |
| `--interface` `<name>` |  |
| `--local-port` `<range>` |  |
| `-4`, `--ipv4` |  |
| `-6`, `--ipv6` |  |
| `--cert-type` `<type>` | Only PEM certificates are supported. |
| `--key-type` `<type>` | Only PEM keys are supported. |
| `--pass` `<phrase>` | Encrypted private keys are not supported. |
| `--capath` `<dir>` | Use `--cacert` with a PEM bundle. |
| `--crlfile` `<file>` |  |
| `--pinnedpubkey` `<hashes>` |  |
| `--ciphers` `<list>` |  |
| `-1`, `--tlsv1` |  |
| `--tlsv1.0` |  |
| `--tlsv1.1` |  |
| `--tlsv1.2` |  |
| `--tlsv1.3` |  |
| `--tls-max` `<version>` |  |
| `--proto` `<protocols>` | Only http and https are supported. |
| `--proto-redir` `<protocols>` | Only http and https are supported. |
| `--limit-rate` `<speed>` |  |
| `--max-filesize` `<bytes>` | Set `max_body` in `sankh.toml`. |
| `--retry-max-time` `<seconds>` |  |
| `--retry-connrefused` |  |
| `--retry-all-errors` |  |
| `--trace` `<file>` |  |
| `--trace-ascii` `<file>` |  |
| `--trace-time` |  |
| `--stderr` `<file>` |  |
| `--libcurl` `<file>` |  |
| `--alt-svc` `<file>` |  |
| `--hsts` `<file>` |  |
| `--doh-url` `<url>` |  |
| `--dns-servers` `<addresses>` |  |
| `--socks5` `<host:port>` | Only HTTP and HTTPS proxies are supported. |
| `--socks5-hostname` `<host:port>` | Only HTTP and HTTPS proxies are supported. |
| `-U`, `--proxy-user` `<user:password>` | Put the credentials in the proxy URL. |
| `--proxy-insecure` |  |
| `-p`, `--proxytunnel` |  |
| `-j`, `--junk-session-cookies` |  |
| `--ignore-content-length` |  |
| `--raw` |  |
| `--tr-encoding` |  |
| `--post301` |  |
| `--post302` |  |
| `--post303` |  |
| `-a`, `--append` |  |
| `-l`, `--list-only` |  |
| `-Q`, `--quote` `<command>` |  |
| `--ssl` |  |
| `--ssl-reqd` |  |
| `-C`, `--continue-at` `<offset>` |  |
| `-z`, `--time-cond` `<time>` | Use `-H "If-Modified-Since: ..."`. |
| `-Z`, `--parallel` | Put each request in its own file. |
| `-n`, `--netrc` | Use `-u "$USER:$PASSWORD"` with variables. |
| `--netrc-optional` | Use `-u "$USER:$PASSWORD"` with variables. |
| `--netrc-file` `<file>` | Use `-u "$USER:$PASSWORD"` with variables. |
| `--variable` `<name=value>` | Define variables in an environment file. |
| `--expand-url` `<url>` | Write `$NAME` in the URL. |

Any other flag is reported as unknown. To ask for one, open an issue at <https://github.com/sankh-dev/sankh/issues>.
