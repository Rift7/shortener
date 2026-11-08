# shortener

[![CI](https://github.com/Rift7/shortener/actions/workflows/ci.yml/badge.svg)](https://github.com/Rift7/shortener/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](rust-toolchain.toml)

A small, self-hostable URL shortener built for agents. Rust + axum + SQLite:
one binary, one file of state, one bearer token to write.

```console
$ curl -sX POST https://go.example.com/api/links \
    -H "authorization: Bearer $KEY" \
    -H 'content-type: application/json' \
    -d '{"url": "https://example.com/a/very/long/path?utm_source=agent"}'
{"slug":"C9hzhWj","short_url":"https://go.example.com/C9hzhWj","url":"https://example.com/a/very/long/path?utm_source=agent","created_at":"2026-11-08T21:14:12.001Z"}
```

## Contents

- [Why this exists](#why-this-exists)
- [API](#api)
- [Quick start](#quick-start)
- [Configuration](#configuration)
- [Deploying](#deploying)
- [Hermes agent integration](#hermes-agent-integration)
- [Design decisions](#design-decisions)
- [Operations](#operations)
- [Development](#development)

## Why this exists

Agent workflows produce links constantly — run logs, generated artifacts,
screenshots, research sources — and those links are long, ugly, and often
unshareable. Existing shorteners are either SaaS products with an API key you
have to trust, or analytics platforms that store visitor data you don't want.

This one is the smallest thing that solves it:

- **Agent-shaped API.** One authenticated `POST`, JSON in and out. No dashboard,
  no OAuth dance. Custom slugs when a link will be read aloud or printed.
- **Self-hosted.** Your links, your domain, your database file.
- **No tracking.** It stores a slug, a target URL, and a timestamp. Nothing else.
- **Small enough to audit.** ~1,100 lines including tests; a single 3.7 MB binary.

## API

| Method | Path | Auth | Behavior |
|---|---|---|---|
| `POST` | `/api/links` | yes | Create a link → `201` |
| `GET` | `/{slug}` | — | `302` to the target |
| `GET` | `/api/links/{slug}` | yes | Inspect a link |
| `GET` | `/healthz` | — | Liveness + link count |

Auth accepts `Authorization: Bearer <key>` or `X-API-Key: <key>`.

### Create a link

```bash
curl -sX POST https://go.example.com/api/links \
  -H "authorization: Bearer $SHORTENER_API_KEY" \
  -H 'content-type: application/json' \
  -d '{"url": "example.com/report", "slug": "report-q4"}'
```

```json
{
  "slug": "report-q4",
  "short_url": "https://go.example.com/report-q4",
  "url": "https://example.com/report",
  "created_at": "2026-11-08T21:14:12.001Z"
}
```

| Field | Required | Rules |
|---|---|---|
| `url` | yes | `http`/`https` only, at most 2048 chars, no embedded credentials. `https://` is assumed when the scheme is omitted. |
| `slug` | no | `[A-Za-z0-9_-]{1,64}`, cannot start with `-`/`_`, cannot shadow a reserved path. Omit for a random 7-char slug. |

Status codes: `201` created, `400` invalid input, `401` missing or bad key,
`409` slug taken, `500` server error.

Errors are uniform:

```json
{"error": "conflict", "message": "slug 'report-q4' is already in use"}
```

### Inspect a link

```bash
curl -s https://go.example.com/api/links/report-q4 \
  -H "authorization: Bearer $SHORTENER_API_KEY"
```

## Quick start

No database server, no config file — environment variables only.

```bash
git clone https://github.com/Rift7/shortener.git
cd shortener

SHORTENER_API_KEYS=$(openssl rand -hex 24) make dev
# → shortener ready bind=127.0.0.1:8080 base_url=http://127.0.0.1:8080 keys=1
```

Or with Docker:

```bash
docker build -t shortener .
docker run --rm -p 127.0.0.1:8080:8080 \
  -e SHORTENER_API_KEYS=$(openssl rand -hex 24) \
  -e SHORTENER_BIND=0.0.0.0:8080 \
  -v "$PWD/data:/data" shortener
```

Then:

```bash
curl -sX POST localhost:8080/api/links \
  -H "authorization: Bearer $SHORTENER_API_KEYS" \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com"}'
```

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `SHORTENER_API_KEYS` | **required** | Comma-separated bearer tokens, so keys rotate without downtime |
| `SHORTENER_BIND` | `127.0.0.1:8080` | Listen address; use `0.0.0.0:8080` inside a container |
| `SHORTENER_PUBLIC_BASE_URL` | `http://127.0.0.1:8080` | Prefix used to build returned `short_url`s |
| `SHORTENER_DB` | `./data/shortener.db` | SQLite path; parent directories are created |
| `SHORTENER_SLUG_LEN` | `7` | Generated slug length, 4–32 |
| `RUST_LOG` | `shortener=info,tower_http=info,warn` | Log filter |

The service refuses to start if `SHORTENER_API_KEYS` is empty or any key is
shorter than 16 characters. See [`.env.example`](.env.example).

## Deploying

The domain must resolve to the host before the proxy can issue a certificate.

**Docker Compose** — shortener plus Caddy with automatic TLS:

```bash
cp .env.example .env      # set SHORT_DOMAIN, ACME_EMAIL, SHORTENER_API_KEYS
docker compose up -d --build
docker compose logs -f caddy     # watch certificate issuance
curl https://go.example.com/healthz
```

**systemd**, no Docker:

```bash
./deploy/deploy.sh root@203.0.113.10 go.example.com you@example.com
```

Builds a musl binary, installs the hardened unit from
[`deploy/shortener.service`](deploy/shortener.service), generates an API key into
`/etc/shortener/shortener.env` (mode `600`), configures Caddy, and prints the key.
Re-running is safe — an existing key is reused.

### Cloudflare DNS

Add an `A` record for the short domain pointing at the VPS.

- **DNS only (grey cloud)** is the simplest path: Caddy completes an HTTP-01
  challenge on port 80 and gets a Let's Encrypt certificate.
- With the **proxy on (orange cloud)**, set SSL/TLS mode to **Full (strict)** and
  use either a Cloudflare Origin CA certificate or a Cloudflare Tunnel instead of
  opening ports. TLS-ALPN validation does not work through the proxy.

## Hermes agent integration

[Hermes](https://github.com/NousResearch/hermes-agent) agents call this service
through a small plugin exposing two tools:

| Tool | Does |
|---|---|
| `shorten_url(url, slug?)` | Creates a link, returns `short_url` |
| `short_url_info(slug)` | Returns target, short URL, creation time |

The plugin source lives in [`plugin/`](plugin/) and installs to
`~/.hermes/plugins/url-shortener/`. Point it at your instance:

```bash
# ~/.hermes/.env
SHORTENER_BASE_URL=https://go.example.com
SHORTENER_API_KEY=<key>
```

```bash
hermes plugins enable url-shortener
```

Tools appear at the start of the next session. Without the env vars they still
appear, but return a clear `not_configured` error instead of failing silently.

Any other agent or script can call the HTTP API directly — the plugin is a thin
wrapper, not a requirement.

## Design decisions

**SQLite, not Postgres.** One file, no server to patch, and WAL mode handles far
more concurrency than a link shortener sees. The whole dataset is a copy.

**Random slugs, not sequential IDs.** Sequential IDs leak how many links exist and
invite enumeration. A 7-character base62 slug is about 3.5 trillion combinations.

**`302`, not `301`.** A permanent redirect is cached by browsers and proxies
effectively forever. A link you cannot fix is worse than a cache miss, so
redirects also carry `Cache-Control: no-store`.

**Only `http`/`https` targets.** Rejecting other schemes means the redirect can
never emit `javascript:` or `file:` URLs, and because the server never fetches a
target it cannot be used as an SSRF probe.

**Constant-time key comparison.** Token comparison must not leak timing
information, and keys never enter the database.

**Env vars, not a config file.** One deployment per instance, twelve-factor
style, so the container and systemd paths stay identical.

## Operations

```bash
# Back up while running (SQLite online backup, safe with WAL)
sqlite3 /opt/shortener/data/shortener.db ".backup '/root/shortener-$(date +%F).db'"

# Rotate a key: add the new one alongside the old, restart, then drop the old
sudo sed -i 's|^SHORTENER_API_KEYS=.*|SHORTENER_API_KEYS=newkey,oldkey|' /etc/shortener/shortener.env
sudo systemctl restart shortener

journalctl -u shortener -f
```

The database file plus its `-wal` and `-shm` companions are the entire state.
Back up all three together, or use `.backup` as above.

## Development

```bash
make dev        # run with a generated key
make test       # unit + router integration tests
make lint       # clippy -D warnings
make fmt        # rustfmt
```

CI runs `cargo fmt --check`, `clippy -D warnings`, `cargo test`, a locked release
build, and a smoke test that creates and follows a real link.

### Layout

```
src/
├── main.rs      # startup, logging, graceful shutdown
├── lib.rs       # state + router construction (shared with tests)
├── config.rs    # env parsing and validation
├── routes.rs    # handlers, auth extractor, routing table
├── db.rs        # SQLite pool, schema, unique-violation helper
├── slug.rs      # slug generation, validation, reserved paths
├── url.rs       # target URL normalization and scheme checks
├── error.rs     # AppError → uniform JSON responses
└── state.rs     # AppState

tests/api.rs     # integration tests driving the real router
deploy/          # systemd unit + one-command VPS deploy
plugin/          # Hermes agent plugin
```

Contributions welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). Security issues:
[SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE) © 2026 Joshua Hedge
