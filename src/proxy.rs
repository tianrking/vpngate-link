use std::{
    io,
    net::{IpAddr, SocketAddr, TcpStream as StdTcpStream, ToSocketAddrs},
    time::Duration,
};

use anyhow::{Context, bail};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, info, warn};

pub async fn serve_relay(addr: SocketAddr, tunnel_device: String) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("TCP relay listening on {addr}, outbound device={tunnel_device}");

    loop {
        let (client, peer) = listener.accept().await?;
        let dev = tunnel_device.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_client(client, peer, &dev).await {
                debug!("proxy client {peer} ended: {err}");
            }
        });
    }
}

async fn handle_client(mut client: TcpStream, peer: SocketAddr, dev: &str) -> anyhow::Result<()> {
    let first = read_exact_vec(&mut client, 1).await?;
    if first[0] == 0x05 {
        handle_socks5(client, dev).await
    } else {
        handle_http(client, first[0], dev).await
    }
    .with_context(|| format!("client {peer}"))
}

async fn handle_socks5(mut client: TcpStream, dev: &str) -> anyhow::Result<()> {
    let methods_count = read_exact_vec(&mut client, 1).await?[0] as usize;
    let _methods = read_exact_vec(&mut client, methods_count).await?;
    client.write_all(&[0x05, 0x00]).await?;

    let head = read_exact_vec(&mut client, 4).await?;
    if head[0] != 0x05 || head[1] != 0x01 {
        client
            .write_all(&[0x05, 0x07, 0, 0x01, 0, 0, 0, 0, 0, 0])
            .await
            .ok();
        bail!("unsupported socks5 command");
    }

    let host = match head[3] {
        0x01 => {
            let raw = read_exact_vec(&mut client, 4).await?;
            IpAddr::from([raw[0], raw[1], raw[2], raw[3]]).to_string()
        }
        0x03 => {
            let len = read_exact_vec(&mut client, 1).await?[0] as usize;
            String::from_utf8_lossy(&read_exact_vec(&mut client, len).await?).to_string()
        }
        0x04 => {
            let raw = read_exact_vec(&mut client, 16).await?;
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&raw);
            IpAddr::from(bytes).to_string()
        }
        _ => bail!("unsupported address type"),
    };
    let port_raw = read_exact_vec(&mut client, 2).await?;
    let port = u16::from_be_bytes([port_raw[0], port_raw[1]]);

    let mut upstream = match connect_via_device(&host, port, dev).await {
        Ok(stream) => stream,
        Err(err) => {
            warn!("SOCKS5 connect {host}:{port} failed: {err}");
            client
                .write_all(&[0x05, 0x04, 0, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .ok();
            return Err(err);
        }
    };

    client
        .write_all(&[0x05, 0x00, 0, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}

async fn handle_http(mut client: TcpStream, first: u8, dev: &str) -> anyhow::Result<()> {
    let header = read_http_header(&mut client, first).await?;
    let header_end = header
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|idx| idx + 4)
        .context("invalid HTTP proxy header")?;
    let (head, rest) = header.split_at(header_end);
    let text = String::from_utf8_lossy(head);
    let mut lines = text.split("\r\n").filter(|line| !line.is_empty());
    let request_line = lines.next().context("missing HTTP request line")?;
    let mut parts = request_line.splitn(3, ' ');
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let version = parts.next().unwrap_or("HTTP/1.1");
    let headers: Vec<&str> = lines.collect();

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = split_host_port(target, 443)?;
        let mut upstream = connect_via_device(&host, port, dev).await?;
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        if !rest.is_empty() {
            upstream.write_all(rest).await?;
        }
        copy_bidirectional(&mut client, &mut upstream).await?;
        return Ok(());
    }

    let (host, port, path) = parse_http_target(target, &headers)?;
    let mut upstream = connect_via_device(&host, port, dev).await?;
    let mut request = format!("{method} {path} {version}\r\n");
    for line in headers {
        let lower = line.to_ascii_lowercase();
        if !lower.starts_with("proxy-connection:") && !lower.starts_with("connection:") {
            request.push_str(line);
            request.push_str("\r\n");
        }
    }
    request.push_str("Connection: close\r\n\r\n");
    upstream.write_all(request.as_bytes()).await?;
    if !rest.is_empty() {
        upstream.write_all(rest).await?;
    }
    copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}

async fn read_exact_vec(stream: &mut TcpStream, len: usize) -> anyhow::Result<Vec<u8>> {
    let mut buf = vec![0; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn read_http_header(stream: &mut TcpStream, first: u8) -> anyhow::Result<Vec<u8>> {
    let mut data = vec![first];
    let mut buf = [0u8; 4096];
    while !data.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if data.len() > 64 * 1024 {
            bail!("HTTP header too large");
        }
    }
    Ok(data)
}

fn parse_http_target(target: &str, headers: &[&str]) -> anyhow::Result<(String, u16, String)> {
    if target.starts_with("http://") || target.starts_with("https://") {
        let uri: http::Uri = target.parse()?;
        let scheme = uri.scheme_str().unwrap_or("http");
        let authority = uri.authority().context("missing URI authority")?.as_str();
        let (host, port) = split_host_port(authority, if scheme == "https" { 443 } else { 80 })?;
        let path = uri
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        return Ok((host, port, path));
    }

    let host_header = headers
        .iter()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        })
        .map(|(_, v)| v.trim())
        .context("missing Host header")?;
    let (host, port) = split_host_port(host_header, 80)?;
    Ok((host, port, target.to_string()))
}

fn split_host_port(value: &str, default_port: u16) -> anyhow::Result<(String, u16)> {
    if let Some(stripped) = value.strip_prefix('[') {
        let end = stripped.find(']').context("invalid IPv6 host")?;
        let host = stripped[..end].to_string();
        let rest = &stripped[end + 1..];
        let port = rest
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(default_port);
        return Ok((host, port));
    }
    if let Some((host, port)) = value.rsplit_once(':')
        && let Ok(port) = port.parse()
    {
        return Ok((host.to_string(), port));
    }
    Ok((value.to_string(), default_port))
}

async fn connect_via_device(host: &str, port: u16, dev: &str) -> anyhow::Result<TcpStream> {
    let host = host.to_string();
    let dev = dev.to_string();
    let std_stream =
        tokio::task::spawn_blocking(move || connect_blocking(&host, port, &dev)).await??;
    std_stream.set_nonblocking(true)?;
    Ok(TcpStream::from_std(std_stream)?)
}

fn connect_blocking(host: &str, port: u16, dev: &str) -> anyhow::Result<StdTcpStream> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    let mut last_err: Option<anyhow::Error> = None;
    for addr in addrs {
        match connect_addr(addr, dev) {
            Ok(stream) => return Ok(stream),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no address resolved for {host}:{port}")))
}

fn connect_addr(addr: SocketAddr, dev: &str) -> anyhow::Result<StdTcpStream> {
    let domain = socket2::Domain::for_address(addr);
    let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    socket.set_read_timeout(Some(Duration::from_secs(20))).ok();
    socket.set_write_timeout(Some(Duration::from_secs(20))).ok();
    bind_to_device(&socket, dev)?;
    socket.connect_timeout(&addr.into(), Duration::from_secs(20))?;
    Ok(socket.into())
}

#[cfg(target_os = "linux")]
fn bind_to_device(socket: &socket2::Socket, dev: &str) -> io::Result<()> {
    use std::{ffi::CString, os::fd::AsRawFd};

    if dev.is_empty() {
        return Ok(());
    }
    let dev = CString::new(dev)?;
    let bytes = dev.as_bytes_with_nul();
    // SAFETY: the file descriptor is owned by `socket`, the option buffer points to a
    // NUL-terminated interface name that remains alive for the syscall, and the length
    // matches the provided buffer.
    let rc = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_BINDTODEVICE,
            bytes.as_ptr().cast(),
            bytes.len() as libc::socklen_t,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "linux"))]
fn bind_to_device(_socket: &socket2::Socket, dev: &str) -> io::Result<()> {
    if !dev.is_empty() {
        warn!("SO_BINDTODEVICE is Linux-only; outbound device {dev} is ignored on this OS");
    }
    Ok(())
}
