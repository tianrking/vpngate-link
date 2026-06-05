#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

VERSION="${VERSION:-$(awk -F\" '/^version = / {print $2; exit}' Cargo.toml)}"
ARCH="${ARCH:-$(dpkg --print-architecture 2>/dev/null || uname -m)}"
case "$ARCH" in
  x86_64) ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
esac

PKG_NAME="vpngate-link"
BUILD_DIR="$ROOT_DIR/target/deb/${PKG_NAME}_${VERSION}_${ARCH}"
OUT_DIR="$ROOT_DIR/target/deb"
DEB_PATH="$OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
RELEASE_DEB_PATH="$OUT_DIR/${PKG_NAME}_${ARCH}.deb"

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required to build a Debian package."
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required."
  exit 1
fi
if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required."
  exit 1
fi

echo "Building React management console..."
if [ -f "$ROOT_DIR/web/package-lock.json" ]; then
  (cd "$ROOT_DIR/web" && npm ci && npm run build)
else
  (cd "$ROOT_DIR/web" && npm install && npm run build)
fi

echo "Building Rust release binary..."
cargo build --release --locked

rm -rf "$BUILD_DIR"
mkdir -p \
  "$BUILD_DIR/DEBIAN" \
  "$BUILD_DIR/opt/vpngate-link/web" \
  "$BUILD_DIR/opt/vpngate-link/data" \
  "$BUILD_DIR/lib/systemd/system" \
  "$BUILD_DIR/etc/default" \
  "$BUILD_DIR/usr/bin"

install -m 0755 "$ROOT_DIR/target/release/vpngate-link" "$BUILD_DIR/opt/vpngate-link/vpngate-link"
cp -R "$ROOT_DIR/web/dist/." "$BUILD_DIR/opt/vpngate-link/web/"
install -m 0644 "$ROOT_DIR/README.md" "$BUILD_DIR/opt/vpngate-link/README.md"
install -m 0644 "$ROOT_DIR/systemd/vpngate-link.service" "$BUILD_DIR/lib/systemd/system/vpngate-link.service"
install -m 0644 "$ROOT_DIR/packaging/default.env" "$BUILD_DIR/etc/default/vpngate-link"
ln -s /opt/vpngate-link/vpngate-link "$BUILD_DIR/usr/bin/vpngate-link"

cat > "$BUILD_DIR/DEBIAN/control" <<EOF
Package: vpngate-link
Version: ${VERSION}
Section: net
Priority: optional
Architecture: ${ARCH}
Maintainer: VPNGate Link Maintainers <root@localhost>
Depends: openvpn, iproute2, iptables, ca-certificates, curl
Description: VPNGate public-node link for relay servers
 VPNGate Link provides a local HTTP/SOCKS5 relay and control UI for
 routing relay-server traffic through a managed tunnel exit.
EOF

cat > "$BUILD_DIR/DEBIAN/conffiles" <<'EOF'
/etc/default/vpngate-link
EOF

cat > "$BUILD_DIR/DEBIAN/postinst" <<'EOF'
#!/usr/bin/env bash
set -e

CONFIG_FILE="/etc/default/vpngate-link"
if grep -q '^VGL_TOKEN=__GENERATE_ON_INSTALL__$' "$CONFIG_FILE"; then
  if command -v openssl >/dev/null 2>&1; then
    TOKEN="$(openssl rand -hex 18)"
  else
    TOKEN="$(date +%s%N | sha256sum | awk '{print $1}' | cut -c1-36)"
  fi
  sed -i "s/^VGL_TOKEN=.*/VGL_TOKEN=${TOKEN}/" "$CONFIG_FILE"
  echo "Generated control token: ${TOKEN}"
fi

mkdir -p /opt/vpngate-link/data /opt/vpngate-link/web
chmod 700 /opt/vpngate-link/data || true

if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload
  systemctl enable vpngate-link.service >/dev/null 2>&1 || true
  systemctl restart vpngate-link.service || true
fi

echo "VPNGate Link installed."
echo "Control UI: http://127.0.0.1:18081"
echo "Relay:      socks5/http://127.0.0.1:19080"
echo "Config:     /etc/default/vpngate-link"
EOF

cat > "$BUILD_DIR/DEBIAN/prerm" <<'EOF'
#!/usr/bin/env bash
set -e
if [ "${1:-}" = "remove" ] || [ "${1:-}" = "deconfigure" ]; then
  if command -v systemctl >/dev/null 2>&1; then
    systemctl stop vpngate-link.service >/dev/null 2>&1 || true
  fi
fi
EOF

cat > "$BUILD_DIR/DEBIAN/postrm" <<'EOF'
#!/usr/bin/env bash
set -e
if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload || true
fi
if [ "${1:-}" = "purge" ]; then
  rm -rf /opt/vpngate-link
fi
EOF

chmod 0755 "$BUILD_DIR/DEBIAN/postinst" "$BUILD_DIR/DEBIAN/prerm" "$BUILD_DIR/DEBIAN/postrm"

mkdir -p "$OUT_DIR"
dpkg-deb --build "$BUILD_DIR" "$DEB_PATH"
cp "$DEB_PATH" "$RELEASE_DEB_PATH"
echo "$DEB_PATH"
echo "$RELEASE_DEB_PATH"
