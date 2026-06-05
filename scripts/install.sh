#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" != "0" ]; then
  echo "Please run as root: sudo bash scripts/install.sh"
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_DIR="${INSTALL_DIR:-/opt/vpngate-link}"
SERVICE_FILE="/etc/systemd/system/vpngate-link.service"
DEFAULT_FILE="/etc/default/vpngate-link"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required. Install Rust first: https://rustup.rs"
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required to build the React management console."
  exit 1
fi

if command -v apt-get >/dev/null 2>&1; then
  apt-get update -q || true
  apt-get install -y openvpn iproute2 iptables ca-certificates curl
elif command -v dnf >/dev/null 2>&1; then
  dnf install -y openvpn iproute iptables ca-certificates curl
elif command -v yum >/dev/null 2>&1; then
  yum install -y openvpn iproute iptables ca-certificates curl
elif command -v apk >/dev/null 2>&1; then
  apk add openvpn iproute2 iptables ca-certificates curl
fi

cd "$ROOT_DIR"

echo "Building React management console..."
(cd "$ROOT_DIR/web" && npm install && npm run build)

echo "Building VPNGate Link backend..."
cargo build --release

install -d "$INSTALL_DIR"
install -d "$INSTALL_DIR/data"
install -d "$INSTALL_DIR/web"
install -d "$(dirname "$DEFAULT_FILE")"
install -m 0755 "$ROOT_DIR/target/release/vpngate-link" "$INSTALL_DIR/vpngate-link"
install -m 0644 "$ROOT_DIR/README.md" "$INSTALL_DIR/README.md"
cp -R "$ROOT_DIR/web/dist/." "$INSTALL_DIR/web/"
install -m 0644 "$ROOT_DIR/systemd/vpngate-link.service" "$SERVICE_FILE"

if [ ! -f "$DEFAULT_FILE" ]; then
  install -m 0644 "$ROOT_DIR/packaging/default.env" "$DEFAULT_FILE"
fi

if grep -q '^VGL_TOKEN=__GENERATE_ON_INSTALL__$' "$DEFAULT_FILE"; then
  if command -v openssl >/dev/null 2>&1; then
    TOKEN="$(openssl rand -hex 18)"
  else
    TOKEN="$(date +%s%N | sha256sum | awk '{print $1}' | cut -c1-36)"
  fi
  sed -i "s/^VGL_TOKEN=.*/VGL_TOKEN=${TOKEN}/" "$DEFAULT_FILE"
else
  TOKEN="$(grep '^VGL_TOKEN=' "$DEFAULT_FILE" 2>/dev/null | cut -d= -f2- || true)"
fi

systemctl daemon-reload
systemctl enable vpngate-link.service
systemctl restart vpngate-link.service

echo "Installed."
echo "Control panel: http://127.0.0.1:18081"
echo "Local relay:   socks5/http://127.0.0.1:19080"
if [ -n "${TOKEN:-}" ]; then
  echo "Token:         $TOKEN"
fi
echo "Use SSH tunnel for remote control:"
echo "  ssh -L 18081:127.0.0.1:18081 root@VPS_IP"
