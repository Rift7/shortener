#!/usr/bin/env bash
# Runs ON THE SERVER. Installs the shortener from an uploaded source tree.
#
#   sudo bash remote-install.sh <domain> <acme-email> <build-dir>
#
# Idempotent: re-running rebuilds the binary, keeps the existing API keys, and
# reloads the service. Written for Debian/Ubuntu with systemd.
set -euo pipefail

DOMAIN="${1:?domain required}"
EMAIL="${2:?acme email required}"
BUILD_DIR="${3:?build dir required}"

if [ "$(id -u)" -eq 0 ]; then SUDO=""; else SUDO="sudo"; fi
log() { printf '\033[36m==>\033[0m %s\n' "$*"; }

if ! command -v systemctl >/dev/null; then
    echo "systemd is required; this script targets Debian/Ubuntu VPS images" >&2
    exit 1
fi

# ---------------------------------------------------------------- build tool
if ! command -v cargo >/dev/null; then
    log "installing Rust toolchain (one-time, ~200 MB)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
fi
# shellcheck disable=SC1090,SC1091
. "$HOME/.cargo/env"

log "building $(uname -m) release binary"
(
    cd "$BUILD_DIR"
    cargo build --release --locked
)
BIN="$BUILD_DIR/target/release/shortener"

# ---------------------------------------------------------------- service user
if ! id -u shortener >/dev/null 2>&1; then
    log "creating system user 'shortener'"
    $SUDO useradd --system --home /opt/shortener --shell /usr/sbin/nologin shortener
fi

$SUDO install -d -m 0755 /opt/shortener
$SUDO install -d -m 0700 -o shortener -g shortener /opt/shortener/data
$SUDO install -m 0755 "$BIN" /opt/shortener/shortener.new
$SUDO mv /opt/shortener/shortener.new /opt/shortener/shortener

# ---------------------------------------------------------------- environment
ENV_FILE=/etc/shortener/shortener.env
$SUDO mkdir -p /etc/shortener
$SUDO touch "$ENV_FILE"
$SUDO chmod 600 "$ENV_FILE"

set_default() {
    local key="$1" value="$2"
    grep -q "^${key}=" "$ENV_FILE" || echo "${key}=${value}" | $SUDO tee -a "$ENV_FILE" >/dev/null
}

set_default SHORTENER_API_KEYS "$(openssl rand -hex 24)"
set_default SHORTENER_BIND "127.0.0.1:8080"
set_default SHORTENER_DB "/opt/shortener/data/shortener.db"
set_default SHORTENER_SLUG_LEN "7"
set_default RUST_LOG "shortener=info,tower_http=info,warn"

# The public base URL always tracks the deployed domain.
$SUDO sed -i "/^SHORTENER_PUBLIC_BASE_URL=/d" "$ENV_FILE"
echo "SHORTENER_PUBLIC_BASE_URL=https://${DOMAIN}" | $SUDO tee -a "$ENV_FILE" >/dev/null

# ---------------------------------------------------------------- systemd
log "installing systemd unit"
$SUDO install -m 0644 "$BUILD_DIR/deploy/shortener.service" /etc/systemd/system/shortener.service
$SUDO systemctl daemon-reload
$SUDO systemctl enable shortener >/dev/null 2>&1 || true
$SUDO systemctl restart shortener

# ---------------------------------------------------------------- caddy
if ! command -v caddy >/dev/null; then
    log "installing Caddy"
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl gnupg
    curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/gpg.key \
        | gpg --dearmor | $SUDO tee /usr/share/keyrings/caddy-stable-archive-keyring.gpg >/dev/null
    curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt \
        | $SUDO tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq caddy
fi

log "writing Caddyfile for ${DOMAIN}"
$SUDO tee /etc/caddy/Caddyfile >/dev/null <<CADDY
{
	email ${EMAIL}
}

${DOMAIN} {
	encode zstd gzip
	header {
		Strict-Transport-Security "max-age=31536000; includeSubDomains"
		X-Content-Type-Options "nosniff"
		-Server
	}
	reverse_proxy 127.0.0.1:8080
	log {
		output stdout
		format console
	}
}
CADDY

$SUDO caddy validate --config /etc/caddy/Caddyfile >/dev/null
$SUDO systemctl enable caddy >/dev/null 2>&1 || true
$SUDO systemctl reload caddy 2>/dev/null || $SUDO systemctl restart caddy

# ---------------------------------------------------------------- verify
log "waiting for the service to answer on loopback"
for _ in $(seq 20); do
    if curl -fsS http://127.0.0.1:8080/healthz >/dev/null 2>&1; then break; fi
    sleep 0.5
done
curl -fsS http://127.0.0.1:8080/healthz || { echo "service did not become healthy" >&2; exit 1; }
echo

log "waiting for the public certificate on https://${DOMAIN}"
for _ in $(seq 30); do
    if curl -fsS "https://${DOMAIN}/healthz" >/dev/null 2>&1; then
        echo "public endpoint is live"
        break
    fi
    sleep 2
done
