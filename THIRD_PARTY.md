# Third-party notices

The Windows build of Sankh embeds [curl 8.23.0_5](https://curl.se/windows/dl-8.23.0_5/curl-8.23.0_5-win64-mingw.zip) (`bin/curl.exe` only) when `curl` is not already installed. The zip sha256 is `b0af3896433242213f6d1388a104115e6dfdd3b16727e5b00c2ab0f48fbc904a`.

That curl is statically linked with LibreSSL, so it does not use the Windows certificate store. Sankh also embeds the Mozilla CA bundle from <https://curl.se/ca/cacert.pem> (data as of Tue Oct 20 13:32:40 2026 GMT) as `curl-ca-bundle.crt`. The file sha256 is `796f1389bfea13c0ed78f663b765baaff54a508e5483786bd6ece73c5e3e3cbe`. When a request uses this bundled curl, Sankh sets `CURL_CA_BUNDLE` to that file unless the variable is already set.

Offline Windows builds can point at local copies. The checksums above still have to match:

- `SANKH_CURL_DIST` — the official zip
- `SANKH_CURL_CA` — the Mozilla `cacert.pem`

## curl

Copyright (c) 1996 - 2026, Daniel Stenberg, <daniel@haxx.se>, and many contributors.

Permission to use, copy, modify, and distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF THIRD PARTY RIGHTS. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall not be used in advertising or otherwise to promote the sale, use or other dealings in this Software without prior written authorization of the copyright holder.

## Libraries linked into that curl.exe

| Library | Version | License |
| --- | --- | --- |
| brotli | 1.2.1 | MIT |
| libpsl | 0.23.3 | MIT |
| LibreSSL | 4.4.0 | OpenSSL and SSLeay licenses, plus ISC for new additions |
| libssh2 | 1.11.1 | BSD-3-Clause |
| nghttp2 | 1.70.0 | MIT |
| nghttp3 | 1.18.0 | MIT |
| ngtcp2 | 1.25.0 | MIT |
| Public Suffix List (psl) | 2026-10-16 | MPL-2.0 |
| zlib-ng | 2.3.3 | zlib |
| zstd | 1.5.7 | BSD-3-Clause |

This product includes software developed by the OpenSSL Project for use in the OpenSSL Toolkit (http://www.openssl.org/).

This product includes cryptographic software written by Eric Young (eay@cryptsoft.com).

This product includes software written by Tim Hudson (tjh@cryptsoft.com).

The Public Suffix List data is covered by the Mozilla Public License 2.0. Full license texts for every library ship in the `dep/` directory of the upstream zip.
