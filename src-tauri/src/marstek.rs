//! Marstek Device Open API (Rev 3.1): JSON-RPC over UDP.

use serde::Serialize;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{timeout_at, Instant};

pub const DEFAULT_PORT: u16 = 30000;

/// The device handles one UDP request at a time, so every exchange takes this lock.
static WIRE: Mutex<()> = Mutex::const_new(());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize, Clone, Debug)]
pub struct Found {
    pub ip: String,
    pub port: u16,
    pub name: Option<String>,
    pub ver: Option<u32>,
}

/// Some firmware answers only to the same source port, so try that port first.
async fn bind(port: u16) -> std::io::Result<UdpSocket> {
    match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).await {
        Ok(s) => Ok(s),
        Err(_) => UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await,
    }
}

fn request(id: u64, method: &str, params: Value) -> Vec<u8> {
    json!({ "id": id, "method": method, "params": params })
        .to_string()
        .into_bytes()
}

/// Receives one datagram. Windows reports an ICMP "port unreachable" as a
/// ConnectionReset on the next recv; that is not a reply, so skip it.
async fn recv(sock: &UdpSocket, buf: &mut [u8], deadline: Instant) -> Result<Option<(usize, SocketAddr)>, String> {
    match timeout_at(deadline, sock.recv_from(buf)).await {
        Err(_) => Ok(None),
        Ok(Ok(r)) => Ok(Some(r)),
        Ok(Err(e)) if e.kind() == ErrorKind::ConnectionReset => Err("unreachable".into()),
        Ok(Err(e)) => Err(e.to_string()),
    }
}

pub async fn call(ip: &str, port: u16, method: &str, params: Value, wait: Duration) -> Result<Value, String> {
    let ip: IpAddr = ip.trim().parse().map_err(|_| format!("invalid IP: {ip}"))?;
    let addr = SocketAddr::new(ip, port);
    let _wire = WIRE.lock().await;
    let sock = bind(port).await.map_err(|e| e.to_string())?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    sock.send_to(&request(id, method, params), addr)
        .await
        .map_err(|e| e.to_string())?;

    let deadline = Instant::now() + wait;
    let mut buf = vec![0u8; 8192];
    loop {
        let Some((n, from)) = recv(&sock, &mut buf, deadline).await? else {
            return Err("timeout".into());
        };
        if from.ip() != ip {
            continue;
        }
        let Ok(reply) = serde_json::from_slice::<Value>(&buf[..n]) else {
            continue;
        };
        // A late reply to an earlier request can still arrive on port 30000.
        if reply.get("id").and_then(Value::as_u64).is_some_and(|r| r != id) {
            continue;
        }
        // Our own request echoed back (same port on this host) is not a reply.
        if reply.get("method").is_some() || (reply.get("result").is_none() && reply.get("error").is_none()) {
            continue;
        }
        if let Some(err) = reply.get("error") {
            let msg = err.get("message").and_then(Value::as_str).unwrap_or("device error");
            let code = err.get("code").and_then(Value::as_i64).unwrap_or(0);
            return Err(format!("{msg} ({code})"));
        }
        return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
    }
}

/// Broadcast targets: the global broadcast plus the directed broadcast of every
/// IPv4 interface, because Windows sends 255.255.255.255 out of one adapter only.
fn broadcast_targets() -> Vec<Ipv4Addr> {
    let mut out = vec![Ipv4Addr::BROADCAST];
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces.into_iter().filter(|i| !i.is_loopback()) {
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                if let Some(b) = v4.broadcast {
                    if !out.contains(&b) {
                        out.push(b);
                    }
                }
            }
        }
    }
    out
}

pub async fn discover(port: u16, wait: Duration) -> Result<Vec<Found>, String> {
    let _wire = WIRE.lock().await;
    let sock = bind(port).await.map_err(|e| e.to_string())?;
    sock.set_broadcast(true).map_err(|e| e.to_string())?;
    let msg = request(0, "Marstek.GetDevice", json!({ "ble_mac": "0" }));
    for target in broadcast_targets() {
        // One unreachable adapter must not stop the scan on the others.
        let _ = sock.send_to(&msg, SocketAddr::new(IpAddr::V4(target), port)).await;
    }

    let deadline = Instant::now() + wait;
    let mut buf = vec![0u8; 8192];
    let mut found: Vec<Found> = Vec::new();
    loop {
        let (n, from) = match recv(&sock, &mut buf, deadline).await {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(_) => continue,
        };
        let Ok(reply) = serde_json::from_slice::<Value>(&buf[..n]) else {
            continue;
        };
        let Some(result) = reply.get("result") else {
            continue;
        };
        let ip = from.ip().to_string();
        if found.iter().any(|d| d.ip == ip) {
            continue;
        }
        found.push(Found {
            ip,
            port,
            name: result.get("device").and_then(Value::as_str).map(String::from),
            ver: result.get("ver").and_then(Value::as_u64).map(|v| v as u32),
        });
    }
    found.sort_by(|a, b| a.ip.cmp(&b.ip));
    Ok(found)
}
