import React from "react";
import { createRoot } from "react-dom/client";
import {
  Activity,
  Ban,
  CheckCircle2,
  Gauge,
  Loader2,
  Play,
  RefreshCw,
  Router,
  Save,
  Search,
  Server,
  Shield,
  SlidersHorizontal,
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

type NodeTestResult = {
  id: string;
  ok: boolean;
  latency_ms: number | null;
  message: string;
};

type SortMode = "recommended" | "latency" | "score" | "speed" | "sessions" | "country";
type FavoriteFilter = "all" | "favorites" | "plain";
type ReachabilityFilter = "all" | "reachable" | "untested" | "failed" | "fast";

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
  const [countryFilter, setCountryFilter] = React.useState("all");
  const [protoFilter, setProtoFilter] = React.useState("all");
  const [statusFilter, setStatusFilter] = React.useState<NodeStatus | "all">("all");
  const [favoriteFilter, setFavoriteFilter] = React.useState<FavoriteFilter>("all");
  const [reachabilityFilter, setReachabilityFilter] = React.useState<ReachabilityFilter>("all");
  const [sortMode, setSortMode] = React.useState<SortMode>("recommended");
  const [scanLimit, setScanLimit] = React.useState(40);
  const [busy, setBusy] = React.useState("");
  const [notice, setNotice] = React.useState("Ready");
  const [exitIp, setExitIp] = React.useState("-");
  const [relayOk, setRelayOk] = React.useState<boolean | null>(null);
  const [authState, setAuthState] = React.useState<"unknown" | "ok" | "error">("unknown");

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
    setAuthState("ok");
  }, [request]);

  React.useEffect(() => {
    loadAll().catch((err) => {
      const message = err instanceof Error ? err.message : String(err);
      setNotice(message);
      setAuthState(message.toLowerCase().includes("unauthorized") ? "error" : "unknown");
    });
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
  const countryOptions = React.useMemo(
    () =>
      Array.from(
        new Map(
          nodes
            .filter((node) => node.country || node.country_short)
            .map((node) => [node.country_short || node.country, `${node.country_short || "XX"} ${node.country}`.trim()])
        ).entries()
      ).sort((a, b) => a[1].localeCompare(b[1])),
    [nodes]
  );
  const protoOptions = React.useMemo(
    () => Array.from(new Set(nodes.map((node) => node.proto).filter(Boolean))).sort(),
    [nodes]
  );
  const summary = React.useMemo(() => {
    const reachable = nodes.filter((node) => node.status === "available" || node.status === "active").length;
    const failed = nodes.filter((node) => node.status === "failed").length;
    const tested = nodes.filter((node) => node.latency_ms !== null || node.status === "failed").length;
    return { reachable, failed, tested };
  }, [nodes]);
  const filteredNodes = React.useMemo(() => {
    const q = filter.trim().toLowerCase();
    const matchesText = (node: Node) => {
      if (!q) return true;
      return `${node.id} ${node.country} ${node.country_short} ${node.host_name} ${node.ip} ${node.remote_host} ${node.remote_port} ${node.proto}`
        .toLowerCase()
        .includes(q);
    };
    const effectiveLatency = (node: Node) => node.latency_ms ?? (node.ping > 0 ? node.ping : null);
    const list = nodes.filter((node) => {
      if (!matchesText(node)) return false;
      if (countryFilter !== "all" && node.country_short !== countryFilter && node.country !== countryFilter) return false;
      if (protoFilter !== "all" && node.proto !== protoFilter) return false;
      if (statusFilter !== "all" && node.status !== statusFilter) return false;
      if (favoriteFilter === "favorites" && !favorites.has(node.id)) return false;
      if (favoriteFilter === "plain" && favorites.has(node.id)) return false;
      if (reachabilityFilter === "reachable" && node.status !== "available" && node.status !== "active") return false;
      if (reachabilityFilter === "untested" && (node.latency_ms !== null || node.status === "failed" || node.status === "available" || node.status === "active")) return false;
      if (reachabilityFilter === "failed" && node.status !== "failed") return false;
      if (reachabilityFilter === "fast") {
        const latency = effectiveLatency(node);
        if (latency === null || latency > 120) return false;
      }
      return true;
    });

    return [...list].sort((a, b) => {
      const latencyA = a.latency_ms ?? (a.ping > 0 ? a.ping : Number.MAX_SAFE_INTEGER);
      const latencyB = b.latency_ms ?? (b.ping > 0 ? b.ping : Number.MAX_SAFE_INTEGER);
      if (sortMode === "latency") return latencyA - latencyB;
      if (sortMode === "score") return b.score - a.score;
      if (sortMode === "speed") return b.speed - a.speed;
      if (sortMode === "sessions") return a.sessions - b.sessions;
      if (sortMode === "country") return `${a.country_short}${a.country}`.localeCompare(`${b.country_short}${b.country}`);
      const statusRank = (node: Node) => (node.status === "active" ? 0 : node.status === "available" ? 1 : node.status === "new" ? 2 : 3);
      return statusRank(a) - statusRank(b) || latencyA - latencyB || b.score - a.score || a.sessions - b.sessions;
    });
  }, [countryFilter, favoriteFilter, favorites, filter, nodes, protoFilter, reachabilityFilter, sortMode, statusFilter]);

  function saveToken() {
    localStorage.setItem("vgl_token", token);
    setNotice("Token saved");
    setAuthState("unknown");
    loadAll().catch((err) => {
      const message = err instanceof Error ? err.message : String(err);
      setNotice(message);
      setAuthState(message.toLowerCase().includes("unauthorized") ? "error" : "unknown");
    });
  }

  function resetFilters() {
    setFilter("");
    setCountryFilter("all");
    setProtoFilter("all");
    setStatusFilter("all");
    setFavoriteFilter("all");
    setReachabilityFilter("all");
    setSortMode("recommended");
    setNotice("Filters reset");
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
      const res = await request<{ result: NodeTestResult }>("/api/test_node", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ id })
      });
      setNotice(`${res.result.ok ? "Reachable" : "Failed"}: ${res.result.message}`);
      await loadAll();
    });
  }

  async function scanFilteredNodes() {
    await run("scan", async () => {
      const ids = filteredNodes.slice(0, scanLimit).map((node) => node.id);
      if (ids.length === 0) {
        setNotice("No routes match current filters");
        return;
      }
      const res = await request<{ tested: number; available: number; results: NodeTestResult[] }>("/api/test_nodes", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ ids })
      });
      setNotice(`Scanned ${res.tested} routes, ${res.available} reachable`);
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
          <span className={`authBadge ${authState}`}>
            {authState === "ok" ? "Authorized" : authState === "error" ? "Token needed" : "Checking"}
          </span>
          <input value={token} onChange={(event) => setToken(event.target.value)} type="password" placeholder="Control token" />
          <button onClick={saveToken}><Save size={16} /> Save</button>
        </div>
      </header>

      <main className="main">
        <section className="metrics">
          <Metric icon={<Router />} label="Active Route" value={activeNode?.country_short || status?.active_node_id || "-"} hint={activeNode?.remote_host || "No tunnel selected"} />
          <Metric icon={<Wifi />} label="Relay" value={status?.relay_addr || "-"} hint={relayOk === null ? "Not checked" : relayOk ? "Listening" : "Down"} />
          <Metric icon={<Server />} label="Routes" value={String(status?.node_count || nodes.length)} hint={`${summary.reachable} reachable, ${favorites.size} favorites`} />
          <Metric icon={<Gauge />} label="Exit IP" value={exitIp} hint={notice} />
        </section>

        <section className="actions">
          <button className="primary" disabled={Boolean(busy)} onClick={refreshRoutes}><RefreshCw size={16} /> Refresh</button>
          <button disabled={Boolean(busy)} onClick={scanFilteredNodes}><CheckCircle2 size={16} /> Scan Visible</button>
          <button disabled={Boolean(busy)} onClick={autoConnect}><Play size={16} /> Auto</button>
          <button disabled={Boolean(busy)} onClick={checkHealth}><Activity size={16} /> Health</button>
          <button disabled={Boolean(busy)} onClick={checkExitIp}><Wifi size={16} /> Exit IP</button>
          <button disabled={Boolean(busy)} onClick={resetFilters}><SlidersHorizontal size={16} /> Reset Filters</button>
          <button className="danger" disabled={Boolean(busy)} onClick={disconnect}><Ban size={16} /> Disconnect</button>
          <div className="search">
            <Search size={16} />
            <input value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="Filter country, endpoint, protocol" />
          </div>
        </section>

        <section className="filters">
          <Field label="Country">
            <select value={countryFilter} onChange={(event) => setCountryFilter(event.target.value)}>
              <option value="all">All countries</option>
              {countryOptions.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
            </select>
          </Field>
          <Field label="Protocol">
            <select value={protoFilter} onChange={(event) => setProtoFilter(event.target.value)}>
              <option value="all">All protocols</option>
              {protoOptions.map((proto) => <option key={proto} value={proto}>{proto.toUpperCase()}</option>)}
            </select>
          </Field>
          <Field label="Status">
            <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value as NodeStatus | "all")}>
              <option value="all">All statuses</option>
              <option value="new">New</option>
              <option value="available">Available</option>
              <option value="failed">Failed</option>
              <option value="active">Active</option>
            </select>
          </Field>
          <Field label="Favorites">
            <select value={favoriteFilter} onChange={(event) => setFavoriteFilter(event.target.value as FavoriteFilter)}>
              <option value="all">All routes</option>
              <option value="favorites">Favorites only</option>
              <option value="plain">Not favorites</option>
            </select>
          </Field>
          <Field label="Reachability">
            <select value={reachabilityFilter} onChange={(event) => setReachabilityFilter(event.target.value as ReachabilityFilter)}>
              <option value="all">Any reachability</option>
              <option value="reachable">Reachable</option>
              <option value="untested">Untested</option>
              <option value="failed">Failed</option>
              <option value="fast">Fast under 120 ms</option>
            </select>
          </Field>
          <Field label="Sort">
            <select value={sortMode} onChange={(event) => setSortMode(event.target.value as SortMode)}>
              <option value="recommended">Recommended</option>
              <option value="latency">Latency</option>
              <option value="score">Score</option>
              <option value="speed">Speed</option>
              <option value="sessions">Low load</option>
              <option value="country">Country</option>
            </select>
          </Field>
          <Field label="Scan limit">
            <input type="number" min="1" max="200" value={scanLimit} onChange={(event) => setScanLimit(Math.max(1, Math.min(200, Number(event.target.value) || 1)))} />
          </Field>
        </section>

        <section className="workspace">
          <div className="panel routes">
            <div className="panelHead">
              <div>
                <h2>Route Catalog</h2>
                <p>{filteredNodes.length} visible routes · {summary.tested} tested · {summary.failed} failed</p>
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
                    <th>Speed</th>
                    <th>Load</th>
                    <th>Test</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredNodes.length === 0 && (
                    <tr>
                      <td className="emptyCell" colSpan={11}>No routes match the current filters.</td>
                    </tr>
                  )}
                  {filteredNodes.map((node) => (
                    <tr key={node.id} className={node.id === status?.active_node_id ? "activeRow" : ""}>
                      <td><button className="mini" disabled={Boolean(busy) || status?.connecting} onClick={() => connectNode(node.id)}>Connect</button></td>
                      <td>
                        <button className={favorites.has(node.id) ? "star on" : "star"} disabled={Boolean(busy)} onClick={() => toggleFavorite(node.id)}>
                          <Star size={16} fill={favorites.has(node.id) ? "currentColor" : "none"} />
                        </button>
                      </td>
                      <td><span className={`pill ${node.status}`}>{node.status}</span></td>
                      <td><strong>{node.country_short || "-"}</strong><span>{node.country}</span></td>
                      <td><code>{node.remote_host}:{node.remote_port}</code><small>{node.id}</small></td>
                      <td>{node.proto}</td>
                      <td>{node.latency_ms ? `${node.latency_ms} ms` : node.ping ? `${node.ping} ms` : "-"}</td>
                      <td>{node.score}</td>
                      <td>{formatSpeed(node.speed)}</td>
                      <td>{node.sessions}</td>
                      <td><button className="mini ghost" disabled={Boolean(busy)} onClick={() => testNode(node.id)}>TCP</button></td>
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
                <select value={settings.country || ""} onChange={(event) => setSettings({ ...settings, country: event.target.value })}>
                  <option value="">Any country</option>
                  {countryOptions.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
                </select>
              </Field>
              <Field label="Fixed node">
                <input value={settings.fixed_node_id} onChange={(event) => setSettings({ ...settings, fixed_node_id: event.target.value })} placeholder="Node ID" />
              </Field>
              <label className="check"><input type="checkbox" checked={settings.connection_enabled} onChange={(event) => setSettings({ ...settings, connection_enabled: event.target.checked })} /> Enable auto connection</label>
              <label className="check"><input type="checkbox" checked={settings.fallback_to_any} onChange={(event) => setSettings({ ...settings, fallback_to_any: event.target.checked })} /> Fallback from favorites</label>
              <button className="primary wide" disabled={Boolean(busy)} onClick={saveSettings}><Save size={16} /> Apply Settings</button>
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

function formatSpeed(value: number) {
  if (!value || value <= 0) return "-";
  const mbps = value / 1_000_000;
  if (mbps >= 100) return `${Math.round(mbps)} Mbps`;
  if (mbps >= 1) return `${mbps.toFixed(1)} Mbps`;
  return `${Math.round(value / 1000)} Kbps`;
}

createRoot(document.getElementById("root")!).render(<App />);
