# sankh.dev

The landing page and docs for Sankh: a static SvelteKit site deployed to
GitHub Pages at <https://sankh.dev>. It also serves the repository's
[`install.sh`](../install.sh) at `https://sankh.dev/install.sh`.

## Development

```bash
npm ci
npm run dev        # http://localhost:5173
npm run check
npm run build      # static output in build/
```

- Landing page: `src/routes/+page.svelte` (code samples in `+page.server.ts`).
- Docs: markdown in `src/content/`, plus `../docs/format.md`, which is read
  directly so the format reference has a single source. The sidebar order lives
  in `src/lib/docs.ts`.
- Code is highlighted at build time with Shiki, so no highlighter ships to the
  browser.
- `og.svg` is the source for `static/og.png`:
  `magick og.svg static/og.png`.

## Deployment

`.github/workflows/pages.yml` builds the site on every push to `main` that
touches `site/`, `docs/` or `install.sh`, copies `install.sh` into the build,
and deploys it with GitHub Pages. Pull requests run the build without
deploying.

### One-time setup

1. Repository **Settings > Pages**: set **Source** to **GitHub Actions**.
2. Organization **Settings > Pages**: verify `sankh.dev` for the `sankh-dev`
   org, so no other account can claim the domain.
3. Repository **Settings > Pages**: set **Custom domain** to `sankh.dev`
   (also committed as `static/CNAME`). Once the certificate is issued, enable
   **Enforce HTTPS**. `.dev` domains only work over HTTPS, so the site is
   unreachable until then; that usually takes a few minutes and can take up to
   an hour.
4. DNS records at your registrar:

   | Type | Name | Value |
   | --- | --- | --- |
   | A | `@` | `185.199.108.153` |
   | A | `@` | `185.199.109.153` |
   | A | `@` | `185.199.110.153` |
   | A | `@` | `185.199.111.153` |
   | AAAA | `@` | `2606:50c0:8000::153` |
   | AAAA | `@` | `2606:50c0:8001::153` |
   | AAAA | `@` | `2606:50c0:8002::153` |
   | AAAA | `@` | `2606:50c0:8003::153` |
   | CNAME | `www` | `sankh-dev.github.io` |

5. After the first deploy, check the installer:

   ```bash
   curl -fsSL https://sankh.dev/install.sh | head -n 3
   ```

### Self-hosting instead

`build/` (plus `install.sh`) is plain static files. To serve it from your own
server, map extensionless URLs to `.html` files and use `404.html` for misses,
e.g. with nginx:

```nginx
root /var/www/sankh.dev;
location / {
    try_files $uri $uri.html $uri/ =404;
}
error_page 404 /404.html;
```
