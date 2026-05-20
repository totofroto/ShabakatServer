use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::net::TcpStream;

const CHECK_IP_ALIVE_TIMEOUT: Duration = Duration::from_millis(300);
const CHECK_IP_ALIVE_PORTS: [u16; 4] = [53, 80, 443, 8080];

pub struct HostProbeResult {
    pub latency_ms: f64,
    pub tcp_ports: Vec<u16>,
}

async fn tcp_knock(addr: SocketAddr) -> Option<u16> {
    let port = addr.port();
    match tokio::time::timeout(CHECK_IP_ALIVE_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => Some(port),
        Ok(Err(e)) => {
            use std::io::ErrorKind;
            if matches!(e.kind(), ErrorKind::ConnectionRefused) {
                Some(port)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// Async TCP-only liveness probe:
/// alive if any configured port either connects or refuses.
pub async fn check_ip_alive(ip: Ipv4Addr) -> Option<Vec<u16>> {
    let ip_std: std::net::IpAddr = ip.into();
    let knock_futures: Vec<_> = CHECK_IP_ALIVE_PORTS
        .iter()
        .map(|&port| tcp_knock(SocketAddr::new(ip_std, port)))
        .collect();
    let responding_ports: Vec<u16> = futures::future::join_all(knock_futures)
        .await
        .into_iter()
        .flatten()
        .collect();
    if responding_ports.is_empty() {
        None
    } else {
        Some(responding_ports)
    }
}

pub async fn ping_host_latency_ms(ip: Ipv4Addr) -> Option<HostProbeResult> {
    let start = std::time::Instant::now();
    let responding_ports = check_ip_alive(ip).await?;
    if responding_ports.is_empty() {
        return None;
    }
    Some(HostProbeResult {
        latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        tcp_ports: responding_ports,
    })
}
