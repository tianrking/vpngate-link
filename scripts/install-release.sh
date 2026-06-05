#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" != "0" ]; then
  echo "Run as root: curl -fsSL <url> | sudo bash"
  exit 1
fi

if ! command -v apt-get >/dev/null 2>&1; then
  echo "This installer currently supports Debian/Ubuntu with apt."
  exit 1
fi

REPO="${VGL_REPO:-tianrking/vpngate-link}"
VERSION="${VGL_VERSION:-latest}"
ARCH="$(dpkg --print-architecture)"
case "$ARCH" in
  amd64|arm64) ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

if [ -n "${VGL_DEB_URL:-}" ]; then
  DEB_URL="$VGL_DEB_URL"
elif [ "$VERSION" = "latest" ]; then
  DEB_URL="https://github.com/${REPO}/releases/latest/download/vpngate-link_${ARCH}.deb"
else
  DEB_URL="https://github.com/${REPO}/releases/download/${VERSION}/vpngate-link_${ARCH}.deb"
fi

TMP_DEB="$(mktemp /tmp/vpngate-link.XXXXXX.deb)"
cleanup() { rm -f "$TMP_DEB"; }
trap cleanup EXIT

apt-get update -q || true
apt-get install -y ca-certificates curl

echo "Downloading $DEB_URL"
curl -fL "$DEB_URL" -o "$TMP_DEB"

apt-get install -y "$TMP_DEB"

TOKEN="$(grep '^VGL_TOKEN=' /etc/default/vpngate-link 2>/dev/null | cut -d= -f2- || true)"
echo
echo "Installed VPNGate Link."
echo "Control UI: http://127.0.0.1:18081"
echo "Relay:      socks5/http://127.0.0.1:19080"
if [ -n "$TOKEN" ]; then
  echo "Token:      $TOKEN"
fi
echo
echo "Remote UI tunnel:"
echo "  ssh -L 18081:127.0.0.1:18081 root@YOUR_VPS_IP"
