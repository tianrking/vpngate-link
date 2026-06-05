import React from "react";
import { createRoot } from "react-dom/client";
import {
  Activity,
  Ban,
  CheckCircle2,
  Gauge,
  Heart,
  Loader2,
  Play,
  RefreshCw,
  Router,
  Save,
  Search,
  Server,
  Shield,
  Star,
  Terminal,
  Wifi
} from "lucide-react";
import "./styles.css";

type NodeStatus = "new" | "available" | "failed" | "active";
type RouteMode = "auto" | "fixed_country" | "fixed_node" | "favorites";

type Node = {
  id: string;
  country: string;
  country_short: string;
  host_name: string;
  ip: string;
  remote_host: string;
  remote_port: number;
  proto: string;
  score: number;
  ping: number;
  speed: number;
  sessions: number;
  latency_ms: number | null;
  status: NodeStatus;
  last_error: string | null;
};

type RuntimeStatus = {
  active_node_id: string | null;
  connecting: boolean;
  relay_addr: string;
  control_addr: string;
  last_message: string;
  last_refresh_at: number | null;
  node_count: number;
  auth_enabled: boolean;
};

type Settings = {
  connection_enabled: boolean;
  route_mode: RouteMode;
  country: string;
  fixed_node_id: string;
  favorite_node_ids: string[];
  fallback_to_any: boolean;
};

type LogEntry = {
  ts: number;
  level: string;
  module: string;
  message: string;
};

const emptySettings: Settings = {
  connection_enabled: true,
  route_mode: "auto",
  country: "",
  fixed_node_id: "",
  favorite_node_ids: [],
  fallback_to_any: true
};

function App() {
  const [token, setToken] = React.useState(localStorage.getItem("vgl_token") || "");
  const [status, setStatus] = React.useState<RuntimeStatus | null>(null);
  const [settings, setSettings] = React.useState<Settings>(emptySettings);
  const [nodes, setNodes] = React.useState<Node[]>([]);
  const [logs, setLogs] = React.useState<LogEntry[]>([]);
  const [filter, setFilter] = React.useState("");
  const [busy, setBusy] = React.useState("");
  const [notice, setNotice] = React.useState("Ready");
  const [exitIp, setExitIp] = React.useState("-");
  const [relayOk, setRelayOk] = React.useState<boolean | null>(null);

  const request = React.useCallback(
    async <T,>(path: string, init: RequestInit = {}): Promise<T> => {
      const headers = new Headers(init.headers || {});
      if (token) headers.set("authorization", `Bearer ${token}`);
      const res = await fetch(path, { ...init, headers });
      const data = await res.json();
      if (!res.ok || data.ok === false) {
        throw new Error(data.error || res.statusText);
      }
      return data;
    },
    [token]
  );

  const loadAll = React.useCallback(async () => {
    const [statusData, nodesData, settingsData, logsData] = await Promise.all([
      request<RuntimeStatus>("/api/status"),
      request<{ nodes: Node[] }>("/api/nodes"),
      request<Settings>("/api/settings"),
      request<{ logs: LogEntry[] }>("/api/logs")
    ]);
    setStatus(statusData);
    setNodes(nodesData.nodes);
    setSettings(settingsData);
    setLogs(logsData.logs.slice(-200).reverse());
    setNotice(statusData.last_message || "Ready");
  }, [request]);

  React.useEffect(() => {
    loadAll().catch((err) => setNotice(err.message));
  }, [loadAll]);

  async function run(label: string, fn: () => Promise<void>) {
    setBusy(label);
    try {
      await fn();
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  }

  const favorites = new Set(settings.favorite_node_ids);
  const activeNode = nodes.find((node) => node.id === status?.active_node_id);
  const filteredNodes = nodes.filter((node) => {
    const q = filter.trim().toLowerCase();
    if (!q) return true;
    return `${node.id} ${node.country} ${node.country_short} ${node.remote_host} ${node.proto}`
      .toLowerCase()
      .includes(q);
  });

  function saveToken() {
    localStorage.setItem("vgl_token", token);
    setNotice("Token saved");
    loadAll().catch((err) => setNotice(err.message));
  }

  async function refreshRoutes() {
    await run("refresh", async () => {
      const res = await request<{ count: number }>("/api/refresh", { method: "POST" });
      setNotice(`Loaded ${res.count} routes`);
      await loadAll();
    });
  }

  async function connectNode(id: string) {
    await run(`connect:${id}`, async () => {
      await request("/api/connect", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ id })
      });
      await loadAll();
    });
  }

  async function testNode(id: string) {
    await run(`test:${id}`, async () => {
      const res = await request<{ result: { message: string } }>("/api/test_node", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ id })
      });
      setNotice(res.result.message);
      await loadAll();
    });
  }

  async function toggleFavorite(id: string) {
    const res = await request<{ settings: Settings }>("/api/favorite", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id })
    });
    setSettings(res.settings);
  }

  async function saveSettings() {
    await run("settings", async () => {
      const res = await request<{ settings: Settings }>("/api/settings", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(settings)
      });
      setSettings(res.settings);
      setNotice("Settings saved");
    });
  }

  async function autoConnect() {
    await run("auto", async () => {
      await request("/api/autoconnect", { method: "POST" });
      await loadAll();
    });
  }

  async function disconnect() {
    await run("disconnect", async () => {
      await request("/api/disconnect", { method: "POST" });
      await loadAll();
    });
  }

  async function checkHealth() {
    await run("health", async () => {
      const health = await request<{ relay: { ok: boolean } }>("/api/health");
      setRelayOk(health.relay.ok);
      setNotice(health.relay.ok ? "Relay is listening" : "Relay is unavailable");
    });
  }

  async function checkExitIp() {
    await run("exit", async () => {
      const res = await request<{ ip: string; latency_ms: number }>("/api/exit_ip");
      setExitIp(`${res.ip} (${res.latency_ms} ms)`);
      setNotice("Exit IP checked");
      await loadAll();
    });
  }

  return (
    <div className="shell">
      <header className="topbar">
        <div className="brand">
          <Shield size={24} />
          <div>
            <h1>VPNGate Link</h1>
            <p>Dynamic exit orchestration for private relay servers</p>
          </div>
        </div>
        <div className="tokenBox">
          <input value={token} onChange={(event) => setToken(event.target.value)} type="password" placeholder="Control token" />
          <button onClick={saveToken}><Save size={16} /> Save</button>
        </div>
      </header>

      <main className="main">
        <section className="metrics">
          <Metric icon={<Router />} label="Active Route" value={activeNode?.country_short || status?.active_node_id || "-"} hint={activeNode?.remote_host || "No tunnel selected"} />
          <Metric icon={<Wifi />} label="Relay" value={status?.relay_addr || "-"} hint={relayOk === null ? "Not checked" : relayOk ? "Listening" : "Down"} />
          <Metric icon={<Server />} label="Routes" value={String(status?.node_count || nodes.length)} hint={`${favorites.size} favorites`} />
          <Metric icon={<Gauge />} label="Exit IP" value={exitIp} hint={notice} />
        </section>

        <section className="actions">
          <button className="primary" onClick={refreshRoutes}><RefreshCw size={16} /> Refresh</button>
          <button onClick={autoConnect}><Play size={16} /> Auto</button>
          <button onClick={checkHealth}><Activity size={16} /> Health</button>
          <button onClick={checkExitIp}><Wifi size={16} /> Exit IP</button>
          <button className="danger" onClick={disconnect}><Ban size={16} /> Disconnect</button>
          <div className="search">
            <Search size={16} />
            <input value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="Filter country, endpoint, protocol" />
          </div>
        </section>

        <section className="workspace">
          <div className="panel routes">
            <div className="panelHead">
              <div>
                <h2>Route Catalog</h2>
                <p>{filteredNodes.length} visible routes</p>
              </div>
              {busy && <span className="busy"><Loader2 size={15} /> {busy}</span>}
            </div>
            <div className="tableWrap">
              <table>
                <thead>
                  <tr>
                    <th>Use</th>
                    <th>Fav</th>
                    <th>Status</th>
                    <th>Country</th>
                    <th>Endpoint</th>
                    <th>Proto</th>
                    <th>Latency</th>
                    <th>Score</th>
                    <th>Load</th>
                    <th>Test</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredNodes.map((node) => (
                    <tr key={node.id} className={node.id === status?.active_node_id ? "activeRow" : ""}>
                      <td><button className="mini" onClick={() => connectNode(node.id)}>Connect</button></td>
                      <td>
                        <button className={favorites.has(node.id) ? "star on" : "star"} onClick={() => toggleFavorite(node.id)}>
                          <Star size={16} fill={favorites.has(node.id) ? "currentColor" : "none"} />
                        </button>
                      </td>
                      <td><span className={`pill ${node.status}`}>{node.status}</span></td>
                      <td><strong>{node.country_short || "-"}</strong><span>{node.country}</span></td>
                      <td><code>{node.remote_host}:{node.remote_port}</code><small>{node.id}</small></td>
                      <td>{node.proto}</td>
                      <td>{node.latency_ms ? `${node.latency_ms} ms` : node.ping ? `${node.ping} ms` : "-"}</td>
                      <td>{node.score}</td>
                      <td>{node.sessions}</td>
                      <td><button className="mini ghost" onClick={() => testNode(node.id)}>TCP</button></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          <aside className="side">
            <div className="panel">
              <div className="panelHead compact">
                <h2>Routing Policy</h2>
              </div>
              <Field label="Mode">
                <select value={settings.route_mode} onChange={(event) => setSettings({ ...settings, route_mode: event.target.value as RouteMode })}>
                  <option value="auto">Auto</option>
                  <option value="fixed_country">Fixed country</option>
                  <option value="fixed_node">Fixed node</option>
                  <option value="favorites">Favorites</option>
                </select>
              </Field>
              <Field label="Country">
                <input value={settings.country} onChange={(event) => setSettings({ ...settings, country: event.target.value })} placeholder="JP, US, Japan" />
              </Field>
              <Field label="Fixed node">
                <input value={settings.fixed_node_id} onChange={(event) => setSettings({ ...settings, fixed_node_id: event.target.value })} placeholder="Node ID" />
              </Field>
              <label className="check"><input type="checkbox" checked={settings.connection_enabled} onChange={(event) => setSettings({ ...settings, connection_enabled: event.target.checked })} /> Enable auto connection</label>
              <label className="check"><input type="checkbox" checked={settings.fallback_to_any} onChange={(event) => setSettings({ ...settings, fallback_to_any: event.target.checked })} /> Fallback from favorites</label>
              <button className="primary wide" onClick={saveSettings}><Save size={16} /> Apply Settings</button>
            </div>

            <div className="panel logs">
              <div className="panelHead compact">
                <h2><Terminal size={16} /> Event Log</h2>
              </div>
              <div className="logList">
                {logs.length === 0 ? <p className="empty">No events yet.</p> : logs.map((log, idx) => (
                  <div className="log" key={`${log.ts}-${idx}`}>
                    <span className={`dot ${log.level.toLowerCase()}`}></span>
                    <div>
                      <strong>{log.module}</strong>
                      <p>{log.message}</p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </aside>
        </section>
      </main>
    </div>
  );
}

function Metric({ icon, label, value, hint }: { icon: React.ReactNode; label: string; value: string; hint: string }) {
  return (
    <div className="metric">
      <div className="metricIcon">{icon}</div>
      <span>{label}</span>
      <strong>{value}</strong>
      <p>{hint}</p>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
