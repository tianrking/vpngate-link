#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != "Linux" ]; then
  echo "live check requires Linux/Ubuntu because tunnel binding uses Linux networking" >&2
  exit 2
fi

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

need curl
need python3
need openvpn

DEFAULT_FILE="${VGL_DEFAULT_FILE:-/etc/default/vpngate-link}"
if [ -f "$DEFAULT_FILE" ]; then
  # shellcheck disable=SC1090
  set -a && . "$DEFAULT_FILE" && set +a
fi

CONTROL="${VGL_CONTROL:-127.0.0.1:18081}"
RELAY="${VGL_RELAY:-127.0.0.1:19080}"
TOKEN="${VGL_TOKEN:-}"
SCAN_LIMIT="${VGL_SCAN_LIMIT:-20}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

auth_args=()
if [ -n "$TOKEN" ]; then
  auth_args=(-H "Authorization: Bearer $TOKEN")
fi

api() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [ -n "$body" ]; then
    curl -fsS "${auth_args[@]}" -H 'content-type: application/json' -X "$method" "http://$CONTROL$path" -d "$body"
  else
    curl -fsS "${auth_args[@]}" -X "$method" "http://$CONTROL$path"
  fi
}

echo "== VPNGate Link live check =="
echo "control: http://$CONTROL"
echo "relay:   socks5/http://$RELAY"

echo "== service status =="
api GET /api/status | tee "$TMP_DIR/status.json" | python3 -m json.tool
api GET /api/health | tee "$TMP_DIR/health.json" | python3 -m json.tool

echo "== direct VPS public IP =="
DIRECT_IP="$(curl -fsS --max-time 12 https://api.ipify.org || true)"
echo "${DIRECT_IP:-unavailable}"

echo "== refresh VPNGate catalog =="
api POST /api/refresh | tee "$TMP_DIR/refresh.json" | python3 -m json.tool
api GET /api/nodes > "$TMP_DIR/nodes.json"

python3 - "$TMP_DIR/nodes.json" "$TMP_DIR/scan-request.json" "$SCAN_LIMIT" <<'PY'
import json
import sys

nodes = json.load(open(sys.argv[1]))["nodes"]
limit = int(sys.argv[3])
tcp_nodes = [n for n in nodes if n.get("proto", "").lower() == "tcp"]
ids = [n["id"] for n in tcp_nodes[:limit]]
print(f"nodes={len(nodes)} tcp_candidates={len(tcp_nodes)} scan_limit={limit}")
for node in tcp_nodes[:5]:
    print(f"candidate {node['id']} {node['country_short']} {node['remote_host']}:{node['remote_port']} ping={node['ping']} score={node['score']}")
json.dump({"ids": ids}, open(sys.argv[2], "w"))
if not ids:
    raise SystemExit("no TCP nodes available to preflight")
PY

echo "== scan TCP candidates =="
api POST /api/test_nodes "@$TMP_DIR/scan-request.json" > "$TMP_DIR/scan.json"
python3 -m json.tool "$TMP_DIR/scan.json"

NODE_ID="$(python3 - "$TMP_DIR/scan.json" <<'PY'
import json
import sys

results = json.load(open(sys.argv[1]))["results"]
for result in results:
    if result.get("ok"):
        print(result["id"])
        break
else:
    raise SystemExit("no reachable TCP node found")
PY
)"
echo "selected: $NODE_ID"

echo "== connect selected node =="
api POST /api/connect "{\"id\":\"$NODE_ID\"}" | tee "$TMP_DIR/connect.json" | python3 -m json.tool

echo "== wait for exit IP through relay =="
EXIT_IP=""
for _ in $(seq 1 12); do
  if EXIT_IP="$(curl -fsS --max-time 12 -x "socks5h://$RELAY" https://api.ipify.org 2>/dev/null)"; then
    break
  fi
  sleep 5
done

if [ -z "$EXIT_IP" ]; then
  echo "unable to read exit IP through relay after connection" >&2
  api GET /api/logs | python3 -m json.tool || true
  exit 1
fi

echo "relay exit IP: $EXIT_IP"
if [ -n "$DIRECT_IP" ] && [ "$DIRECT_IP" = "$EXIT_IP" ]; then
  echo "warning: relay exit IP equals direct VPS IP; verify OpenVPN tunnel and policy routing" >&2
  exit 1
fi

echo "== API exit IP check =="
api GET /api/exit_ip | python3 -m json.tool

echo "live check passed"
