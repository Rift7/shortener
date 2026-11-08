# Security Policy

## Reporting a vulnerability

Do not open a public issue.

Use GitHub's private reporting:
**https://github.com/Rift7/shortener/security/advisories/new**

Include the affected version or commit, reproduction steps, and the impact you
believe it has. You can expect an initial response within 72 hours.

## Supported versions

The `main` branch and the most recent tagged release receive fixes. Older tags
do not.

## Threat model

This service is designed to sit behind a TLS-terminating reverse proxy on a
small host. Its security properties are:

- **Write access requires an API key.** `POST /api/links` and
  `GET /api/links/{slug}` reject requests without a valid bearer token or
  `X-API-Key`. Keys are compared in constant time and are never written to the
  database.
- **Only `http`/`https` targets.** Scheme validation prevents the redirect from
  emitting `javascript:`, `data:`, or `file:` URLs that would execute in a
  visitor's browser or expose local files.
- **Stored redirects only.** The service never fetches a target URL, so it
  cannot be used as an SSRF probe.
- **No anonymous writes.** There is no public API surface that creates or
  mutates links.

Out of scope, by design:

- Click analytics and visitor tracking (the service stores no request data).
- Abuse prevention for the redirect path. Deployments that publish the link
  domain should rate-limit at the proxy or CDN.
- Multi-tenant isolation. One instance serves one operator; use separate
  instances and separate databases for separate trust domains.

## Hardening checklist for operators

- Terminate TLS at a proxy (Caddy, Cloudflare, or equivalent); never serve the
  API over plain HTTP on a public interface.
- Keep `SHORTENER_BIND` on `127.0.0.1` when a proxy runs on the same host.
- Generate keys with `openssl rand -hex 24`. Rotate by listing the new and old
  key comma-separated in `SHORTENER_API_KEYS`, then dropping the old one.
- Keep the SQLite file at `0600` and include it in backups.
