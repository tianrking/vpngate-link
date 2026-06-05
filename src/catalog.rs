use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use base64::Engine;
use csv::StringRecord;
use reqwest::header::{ACCEPT, CACHE_CONTROL, PRAGMA, USER_AGENT};
use tokio::process::Command;

use crate::{
    config::Config,
    model::{Node, NodeStatus},
};

pub async fn fetch_nodes(cfg: &Config) -> anyhow::Result<Vec<Node>> {
    let text = fetch_catalog_text(cfg).await?;

    parse_catalog_csv(&text, cfg.max_nodes)
}

async fn fetch_catalog_text(cfg: &Config) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .http1_only()
        .user_agent("Mozilla/5.0 (compatible; VPNGate Link)")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let reqwest_result = match client
        .get(&cfg.catalog_url)
        .header(USER_AGENT, "Mozilla/5.0 (compatible; VPNGate Link)")
        .header(ACCEPT, "text/plain, text/csv, */*")
        .header(CACHE_CONTROL, "no-cache")
        .header(PRAGMA, "no-cache")
        .send()
        .await
    {
        Ok(res) => match res.error_for_status() {
            Ok(res) => res.text().await,
            Err(err) => Err(err),
        },
        Err(err) => Err(err),
    };

    match reqwest_result {
        Ok(text) => Ok(text),
        Err(reqwest_err) => fetch_catalog_text_with_curl(cfg)
            .await
            .with_context(|| format!("reqwest fetch failed first: {reqwest_err}")),
    }
}

async fn fetch_catalog_text_with_curl(cfg: &Config) -> anyhow::Result<String> {
    let output = Command::new("curl")
        .arg("-fsSL")
        .arg("--http1.1")
        .arg("--max-time")
        .arg("25")
        .arg("-A")
        .arg("Mozilla/5.0 (compatible; VPNGate Link)")
        .arg(&cfg.catalog_url)
        .output()
        .await
        .context("failed to run curl fallback for catalog fetch")?;
    if !output.status.success() {
        bail!(
            "curl fallback failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn parse_catalog_csv(text: &str, max_nodes: usize) -> anyhow::Result<Vec<Node>> {
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('*'))
        .map(|line| line.to_string())
        .collect();
    if lines.is_empty() {
        bail!("upstream catalog returned an empty list");
    }
    if let Some(first) = lines.first_mut()
        && first.starts_with('#')
    {
        *first = first.trim_start_matches('#').to_string();
    }

    let csv_text = lines.join("\n");
    let mut reader = csv::Reader::from_reader(csv_text.as_bytes());
    let headers = reader.headers()?.clone();
    let mut nodes = Vec::new();

    for rec in reader.records().take(max_nodes) {
        let rec = rec?;
        if let Some(node) = row_to_node(&headers, &rec)? {
            nodes.push(node);
        }
    }

    if nodes.is_empty() {
        bail!("upstream catalog did not contain usable tunnel entries");
    }
    nodes.sort_by_key(|n| (n.ping <= 0, n.ping, -n.score));
    Ok(nodes)
}

fn row_to_node(headers: &StringRecord, rec: &StringRecord) -> anyhow::Result<Option<Node>> {
    let get = |name: &str| -> &str {
        headers
            .iter()
            .position(|h| h == name)
            .and_then(|idx| rec.get(idx))
            .unwrap_or("")
    };

    let encoded = get("OpenVPN_ConfigData_Base64");
    if encoded.is_empty() {
        return Ok(None);
    }
    let config_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .context("failed to decode OpenVPN config")?;
    let config_text = String::from_utf8_lossy(&config_bytes).to_string();

    let (remote_host, remote_port, proto) = parse_remote(&config_text, get("IP"));
    if remote_host.is_empty() || remote_port == 0 {
        return Ok(None);
    }

    let country_short = get("CountryShort").to_string();
    let ip = get("IP").to_string();
    let id = safe_id(&format!(
        "{}_{}_{}_{}",
        if country_short.is_empty() {
            "XX"
        } else {
            &country_short
        },
        if ip.is_empty() { &remote_host } else { &ip },
        remote_port,
        proto
    ));

    Ok(Some(Node {
        id,
        country: get("CountryLong").to_string(),
        country_short,
        host_name: get("HostName").to_string(),
        ip,
        remote_host,
        remote_port,
        proto,
        score: parse_i64(get("Score")),
        ping: parse_i64(get("Ping")),
        speed: parse_i64(get("Speed")),
        sessions: parse_i64(get("NumVpnSessions")),
        config_text,
        latency_ms: None,
        status: NodeStatus::New,
        last_error: None,
    }))
}

pub fn parse_remote(config: &str, fallback_host: &str) -> (String, u16, String) {
    let mut host = fallback_host.to_string();
    let mut port = 0;
    let mut proto = "unknown".to_string();

    for raw in config.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0].to_ascii_lowercase().as_str() {
            "proto" if parts.len() >= 2 => proto = parts[1].to_ascii_lowercase(),
            "remote" if parts.len() >= 3 => {
                host = parts[1].to_string();
                port = parts[2].parse().unwrap_or(0);
                if parts.len() >= 4 {
                    proto = parts[3].to_ascii_lowercase();
                }
            }
            _ => {}
        }
    }

    (host, port, proto)
}

fn safe_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches(['_', '.', '-']);
    if trimmed.is_empty() {
        format!(
            "node_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        )
    } else {
        trimmed.to_string()
    }
}

fn parse_i64(value: &str) -> i64 {
    value.parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use super::*;

    #[test]
    fn parses_remote_line() {
        let cfg = "client\nproto tcp\nremote 203.0.113.10 443\n";
        let (host, port, proto) = parse_remote(cfg, "");
        assert_eq!(host, "203.0.113.10");
        assert_eq!(port, 443);
        assert_eq!(proto, "tcp");
    }

    #[test]
    fn parses_catalog_csv() {
        let conf = "client\nproto udp\nremote 198.51.100.1 1194\n";
        let encoded = base64::engine::general_purpose::STANDARD.encode(conf);
        let body = format!(
            "*vpn_servers\n#HostName,IP,Score,Ping,Speed,CountryLong,CountryShort,NumVpnSessions,Uptime,TotalUsers,TotalTraffic,LogType,Operator,Message,OpenVPN_ConfigData_Base64\nhost1,198.51.100.1,100,20,3000,Japan,JP,4,1,1,1,2weeks,op,msg,{encoded}\n"
        );
        let nodes = parse_catalog_csv(&body, 10).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].remote_port, 1194);
        assert_eq!(nodes[0].country_short, "JP");
    }
}
