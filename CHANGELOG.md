# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-08

### Added

- `POST /api/links` — create short links with generated or caller-supplied slugs.
- `GET /{slug}` — `302` redirect with `Cache-Control: no-store` and
  `Referrer-Policy: no-referrer`.
- `GET /api/links/{slug}` — inspect an existing link.
- `GET /healthz` — liveness probe with link count.
- Bearer-token and `X-API-Key` authentication with constant-time comparison and
  comma-separated keys for rotation.
- Target URL validation: `http`/`https` only, scheme inference, length and
  credential checks.
- SQLite storage with WAL mode, unique slug constraint, and collision retry.
- Graceful shutdown on `SIGINT`/`SIGTERM`.
- Docker image (multi-stage), Compose stack with Caddy automatic TLS, hardened
  systemd unit, and `deploy/deploy.sh` for one-command VPS installation.
- Hermes agent plugin exposing `shorten_url` and `short_url_info` tools.
- CI: formatting, clippy, tests, locked release build, binary smoke test.

[Unreleased]: https://github.com/Rift7/shortener/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Rift7/shortener/releases/tag/v0.1.0
