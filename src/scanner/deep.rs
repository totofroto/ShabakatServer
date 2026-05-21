use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::sync::Semaphore;
use std::sync::OnceLock;

static PORT_SCAN_SEM: OnceLock<Semaphore> = OnceLock::new();

pub fn port_scan_sem() -> &'static Semaphore {
    PORT_SCAN_SEM.get_or_init(|| Semaphore::new(32))
}

pub async fn scan_single_port(ip: IpAddr, port: u16) -> Option<u16> {
    let _permit = port_scan_sem().acquire().await.ok()?;
    
    let addr = SocketAddr::new(ip, port);
    match tokio::time::timeout(Duration::from_millis(1500), tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_)) => Some(port),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // Some systems might RST quickly, but it still means the port is "there"
            // though usually we only care about open ports.
            // For Deep Scan we usually want actually open ports.
            None
        }
        _ => None,
    }
}

pub async fn scan_ports(ip: IpAddr, ports: &[u16]) -> Vec<u16> {
    let mut tasks = Vec::new();
    for &port in ports {
        tasks.push(scan_single_port(ip, port));
    }
    
    futures::future::join_all(tasks)
        .await
        .into_iter()
        .flatten()
        .collect()
}
