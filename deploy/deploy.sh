#!/usr/bin/env bash
# Deploy the shortener to a VPS from this checkout.
#
#   ./deploy/deploy.sh root@203.0.113.10 go.example.com you@example.com
#
# Uploads the tracked source tree and builds it on the server, so architecture
# and libc always match (no cross-compilation). Installs a hardened systemd
# unit, puts Caddy in front for automatic TLS, and prints the API key.
set -euo pipefail

HOST="${1:?usage: deploy.sh user@host domain [acme-email]}"
DOMAIN="${2:?usage: deploy.sh user@host domain [acme-email]}"
EMAIL="${3:-postmaster@${DOMAIN#*.}}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SSH_OPTS=(-o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10)
if [ -f "$HOME/.ssh/id_ed25519_shortener" ]; then
    SSH_OPTS+=(-i "$HOME/.ssh/id_ed25519_shortener")
fi
SSH=(ssh "${SSH_OPTS[@]}" "$HOST")
SCP=(scp "${SSH_OPTS[@]}")

say() { printf '\033[36m==>\033[0m %s\n' "$*"; }

if [ -n "$(git status --porcelain)" ]; then
    echo "error: working tree has uncommitted changes; commit or stash before deploying" >&2
    git status --short >&2
    exit 1
fi

say "checking SSH access to $HOST"
"${SSH[@]}" 'echo "    connected as $(whoami) on $(hostname) ($(uname -m))"'

BUILD_DIR="/tmp/shortener-build-$$"

say "uploading source (tracked files only)"
git archive --format=tar.gz HEAD | "${SSH[@]}" "mkdir -p '$BUILD_DIR' && tar xzf - -C '$BUILD_DIR'"

say "uploading installer"
"${SCP[@]}" deploy/remote-install.sh "$HOST:/tmp/shortener-remote-install.sh"

say "installing on $HOST (builds there; the first run downloads Rust)"
"${SSH[@]}" "sudo bash /tmp/shortener-remote-install.sh '$DOMAIN' '$EMAIL' '$BUILD_DIR' \
    && rm -rf '$BUILD_DIR' /tmp/shortener-remote-install.sh"

echo
say "API key — add this to ~/.hermes/.env as SHORTENER_API_KEY:"
"${SSH[@]}" "sudo grep '^SHORTENER_API_KEYS=' /etc/shortener/shortener.env | cut -d= -f2- | cut -d, -f1"

echo
say "done. Verify from anywhere with:"
echo "    curl https://$DOMAIN/healthz"
