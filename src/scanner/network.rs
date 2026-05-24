use std::net::{IpAddr, Ipv4Addr};
use std::sync::OnceLock;
use std::time::Duration;

use ipnet::Ipv4Net;
use log::info;
use tokio::sync::Semaphore;

/// glibc is thread-safe; no need for the tight Android/Bionic limit of 4.
const OS_DNS_MAX_CONCURRENCY: usize = 32;
const OS_DNS_LOOKUP_TIMEOUT: Duration = Duration::from_millis(800);

static OS_DNS_SEM: OnceLock<Semaphore> = OnceLock::new();

pub(crate) fn os_dns_sem() -> &'static Semaphore {
    OS_DNS_SEM.get_or_init(|| Semaphore::new(OS_DNS_MAX_CONCURRENCY))
}

pub struct LocalNetwork {
    pub interface_ip: Ipv4Addr,
    pub cidr: Ipv4Net,
}

/// Fallback: derive the IPv4 via the UDP-connect trick and choose a sensible default prefix.
pub async fn fallback_ipv4_network_via_udp() -> Result<LocalNetwork, String> {
    tokio::task::spawn_blocking(|| {
        let ip = super::get_best_local_ip()
            .ok_or_else(|| "udp fallback: no local address".to_string())?;

        // Use detected local IP with a default /24 prefix
        let prefix = 24;
        info!(
            "scan: UDP fallback → {ip} (getifaddrs blocked); using default /24 prefix",
            ip = ip
        );

        let cidr = Ipv4Net::new(ip, prefix).map_err(|e| format!("udp fallback cidr: {e}"))?;
        Ok(LocalNetwork {
            interface_ip: ip,
            cidr,
        })
    })
    .await
    .map_err(|e| format!("udp fallback task: {e}"))?
}

/// LAN discovery using `default-net` (finds the default or WiFi/Ethernet interface).
pub async fn local_ipv4_network() -> Result<LocalNetwork, String> {
    tokio::task::spawn_blocking(local_ipv4_network_desktop)
        .await
        .map_err(|e| format!("interface lookup panicked: {e}"))?
}

fn local_ipv4_network_desktop() -> Result<LocalNetwork, String> {
    use default_net::interface::InterfaceType;
    use default_net::{get_default_interface, get_interfaces};

    let interfaces = get_interfaces();

    // Prefer WiFi on dev machines; on a wired NAS, fall through to default interface.
    let wifi_iface = interfaces.into_iter().find(|iface| {
        iface.if_type == InterfaceType::Wireless80211
            && iface.name != "lo"
            && iface
                .ipv4
                .first()
                .map(|n| !n.addr.is_loopback())
                .unwrap_or(false)
    });

    if let Some(iface) = wifi_iface {
        let net = iface
            .ipv4
            .first()
            .ok_or_else(|| "Wi‑Fi interface has no IPv4 network".to_string())?;
        let cidr = Ipv4Net::new(net.addr, net.prefix_len)
            .map_err(|err| format!("invalid Wi‑Fi IPv4 network: {err}"))?;
        return Ok(LocalNetwork {
            interface_ip: net.addr,
            cidr,
        });
    }

    let interface =
        get_default_interface().map_err(|err| format!("default interface lookup failed: {err}"))?;
    let network = interface
        .ipv4
        .first()
        .ok_or_else(|| "default interface has no IPv4 network".to_string())?;

    let cidr = Ipv4Net::new(network.addr, network.prefix_len)
        .map_err(|err| format!("invalid local network: {err}"))?;

    Ok(LocalNetwork {
        interface_ip: network.addr,
        cidr,
    })
}

pub fn host_ips(cidr: &Ipv4Net) -> Vec<Ipv4Addr> {
    cidr.hosts().collect()
}

/// Reverse-DNS lookup via the OS resolver, guarded by `OS_DNS_SEM`.
pub async fn reverse_dns(ip: Ipv4Addr) -> Option<String> {
    let _permit = os_dns_sem().acquire().await.ok()?;

    let result = tokio::time::timeout(
        OS_DNS_LOOKUP_TIMEOUT,
        tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&IpAddr::V4(ip))),
    )
    .await
    .ok()?
    .ok()?
    .ok()?;

    let cleaned = result.trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}
