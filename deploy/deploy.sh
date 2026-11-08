#!/usr/bin/env bash
# Deploy the shortener to a VPS over SSH.
#
#   ./deploy/deploy.sh root@203.0.113.10 go.example.com
#
# Builds a static-ish release binary locally, copies it up with its systemd
# unit, and installs a Caddy reverse proxy on the remote host for TLS.
set -euo pipefail

HOST="${1:?usage: deploy.sh user@host domain [acme-email]}"
DOMAIN="${2:?usage: deploy.sh user@host domain [acme-email]}"
EMAIL="${3:-admin@${DOMAIN#*.}}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> building release binary for linux/amd64"
if ! command -v cargo-zigbuild >/dev/null && [ "$(uname -s)" = "Darwin" ]; then
  echo "    (installing cross-compilation target for musl)"
  rustup target add x86_64-unknown-linux-musl
fi
# Native build when the local arch matches the VPS; otherwise use musl.
TARGET=x86_64-unknown-linux-musl
cargo build --release --target "$TARGET"

echo "==> copying artifacts to $HOST"
ssh "$HOST" "mkdir -p /opt/shortener/data /etc/shortener && id -u shortener >/dev/null 2>&1 || useradd --system --home /opt/shortener --shell /usr/sbin/nologin shortener"
scp "target/$TARGET/release/shortener" "$HOST:/opt/shortener/shortener.new"
scp deploy/shortener.service "$HOST:/etc/systemd/system/shortener.service"

echo "==> writing environment file (keeps existing API key if present)"
ssh "$HOST" "set -e
  touch /etc/shortener/shortener.env
  chmod 600 /etc/shortener/shortener.env
  grep -q '^SHORTENER_API_KEYS=' /etc/shortener/shortener.env || echo \"SHORTENER_API_KEYS=\$(openssl rand -hex 24)\" >> /etc/shortener/shortener.env
  grep -q '^SHORTENER_BIND=' /etc/shortener/shortener.env || echo 'SHORTENER_BIND=127.0.0.1:8080' >> /etc/shortener/shortener.env
  grep -q '^SHORTENER_DB=' /etc/shortener/shortener.env || echo 'SHORTENER_DB=/opt/shortener/data/shortener.db' >> /etc/shortener/shortener.env
  sed -i '/^SHORTENER_PUBLIC_BASE_URL=/d' /etc/shortener/shortener.env
  echo 'SHORTENER_PUBLIC_BASE_URL=https://${DOMAIN}' >> /etc/shortener/shortener.env
  chown shortener:shortener /opt/shortener/data
  mv /opt/shortener/shortener.new /opt/shortener/shortener
  chmod 755 /opt/shortener/shortener
  systemctl daemon-reload
  systemctl enable --now shortener
  systemctl restart shortener
"

echo "==> ensuring Caddy is installed and proxying $DOMAIN"
ssh "$HOST" "set -e
  if ! command -v caddy >/dev/null; then
    apt-get update -qq
    apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl gnupg
    curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/gpg.key | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt > /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq && apt-get install -y -qq caddy
  fi
  cat > /etc/caddy/Caddyfile <<EOF
${DOMAIN} {
    encode zstd gzip
    header {
        Strict-Transport-Security \"max-age=31536000; includeSubDomains\"
        X-Content-Type-Options \"nosniff\"
        -Server
    }
    reverse_proxy 127.0.0.1:8080
    log {
        output stdout
        format console
    }
}
EOF
  caddy validate --config /etc/caddy/Caddyfile >/dev/null
  systemctl reload caddy 2>/dev/null || systemctl restart caddy
"

echo "==> health check"
ssh "$HOST" "for i in \$(seq 15); do curl -fsS http://127.0.0.1:8080/healthz && break || sleep 1; done"
echo
echo "==> API key (store this in Hermes' .env as SHORTENER_API_KEY):"
ssh "$HOST" "grep '^SHORTENER_API_KEYS=' /etc/shortener/shortener.env | cut -d= -f2-"
echo
echo "Point an A record for ${DOMAIN} at this host, then: curl https://${DOMAIN}/healthz"
