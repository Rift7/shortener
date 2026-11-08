"""Hermes plugin: shorten URLs through a self-hosted Rust link shortener.

Env:
    SHORTENER_BASE_URL  e.g. https://s.example.com (default http://127.0.0.1:8080)
    SHORTENER_API_KEY   bearer token accepted by the service
"""

import json
import os
import urllib.error
import urllib.request

DEFAULT_BASE_URL = "http://127.0.0.1:8080"
TIMEOUT_SECONDS = 10


def _config():
    base = (os.getenv("SHORTENER_BASE_URL") or DEFAULT_BASE_URL).rstrip("/")
    key = (os.getenv("SHORTENER_API_KEY") or "").strip()
    return base, key


def _request(method, path, payload=None, auth=True):
    base, key = _config()

    if auth and not key:
        return {
            "success": False,
            "error": "not_configured",
            "message": (
                "SHORTENER_API_KEY is not set. Add SHORTENER_BASE_URL and "
                "SHORTENER_API_KEY to ~/.hermes/.env and restart Hermes."
            ),
        }

    data = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(f"{base}{path}", data=data, method=method)
    req.add_header("content-type", "application/json")
    if auth:
        req.add_header("authorization", f"Bearer {key}")

    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT_SECONDS) as resp:
            body = resp.read()
            parsed = json.loads(body) if body else {}
            return {"success": True, **parsed}
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        try:
            detail = json.loads(raw)
        except ValueError:
            detail = {"message": raw[:300]}
        return {
            "success": False,
            "status": exc.code,
            "error": detail.get("error", f"http_{exc.code}"),
            "message": detail.get("message", ""),
        }
    except Exception as exc:  # noqa: BLE001 - surfaced to the model as data
        return {
            "success": False,
            "error": "request_failed",
            "message": f"{type(exc).__name__}: {exc}",
        }


def shorten_url(params, **_kwargs):
    url = (params.get("url") or "").strip()
    if not url:
        return json.dumps({"success": False, "error": "invalid_request", "message": "url is required"})

    payload = {"url": url}
    slug = (params.get("slug") or "").strip()
    if slug:
        payload["slug"] = slug

    return json.dumps(_request("POST", "/api/links", payload))


def short_url_info(params, **_kwargs):
    slug = (params.get("slug") or "").strip().lstrip("/")
    if not slug:
        return json.dumps({"success": False, "error": "invalid_request", "message": "slug is required"})
    return json.dumps(_request("GET", f"/api/links/{slug}"))


def register(ctx):
    ctx.register_tool(
        name="shorten_url",
        toolset="url_shortener",
        schema={
            "name": "shorten_url",
            "description": (
                "Create a short link on the self-hosted short domain. Returns "
                "the short_url to share, the normalized target url, and the slug. "
                "Pass slug to request a memorable ending (letters, digits, - and _); "
                "omit it to get a random one. A slug that is already taken returns "
                "a conflict error instead of overwriting."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Target URL. https:// is assumed when the scheme is omitted.",
                    },
                    "slug": {
                        "type": "string",
                        "description": "Optional custom slug (1-64 chars of A-Za-z0-9_-).",
                    },
                },
                "required": ["url"],
            },
        },
        handler=shorten_url,
    )

    ctx.register_tool(
        name="short_url_info",
        toolset="url_shortener",
        schema={
            "name": "short_url_info",
            "description": (
                "Look up an existing short link by its slug: returns the target "
                "URL, full short_url, and creation timestamp."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "Slug to inspect, with or without a leading slash.",
                    }
                },
                "required": ["slug"],
            },
        },
        handler=short_url_info,
    )
