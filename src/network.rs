use std::collections::HashMap;
use std::future::Future;
use std::fs::read_dir;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;

use futures_util::stream::{Stream, StreamExt};
use rtnetlink::{new_multicast_connection, MulticastGroup};
use tokio::fs::read_to_string;
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsState {
    Missing,
    Stub,
    Direct,
    Mixed,
}

impl DnsState {
    /// Short DNS label for compact output.
    pub fn compact_label(&self) -> &'static str {
        match self {
            Self::Missing => "no-dns",
            Self::Stub => "stub",
            Self::Direct => "dns",
            Self::Mixed => "mixed",
        }
    }

    /// Verbose DNS label for full output.
    pub fn full_label(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Stub => "stub",
            Self::Direct => "direct",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceKind {
    Wireless,
    Wired,
    Tunnel,
    Other,
}

impl InterfaceKind {
    /// Single-letter kind label for compact output.
    pub fn compact_label(&self) -> &'static str {
        match self {
            Self::Wireless => "W",
            Self::Wired => "E",
            Self::Tunnel => "T",
            Self::Other => "N",
        }
    }

    /// Full kind label for verbose output.
    pub fn full_label(&self) -> &'static str {
        match self {
            Self::Wireless => "wifi",
            Self::Wired => "ethernet",
            Self::Tunnel => "tunnel",
            Self::Other => "other",
        }
    }

}

#[derive(Clone, Debug, Default)]
pub struct DevStats {
    pub recv_bytes: u128,
    pub trans_bytes: u128,
}

#[derive(Clone, Debug)]
pub struct InterfaceState {
    pub name: String,
    pub kind: InterfaceKind,
    pub link_up: bool,
}

#[derive(Clone, Debug)]
pub struct ConnectivitySnapshot {
    pub primary_iface: Option<InterfaceState>,
    pub has_default_route: bool,
    pub dns_state: DnsState,
}

/// Subscribe to passive kernel network events.
pub fn spawn_connectivity_events() -> Result<Arc<Notify>, String> {
    let (connection, _handle, messages) = match new_multicast_connection(&_connectivity_groups()) {
        Ok(connection) => connection,
        Err(err) => return Err(format!("Failed to open rtnetlink monitor: {err}")),
    };
    let notify = Arc::new(Notify::new());
    let event_notify = notify.clone();

    tokio::spawn(_run_connectivity_monitor(connection));
    tokio::spawn(_forward_connectivity_events(messages, event_notify));

    Ok(notify)
}

/// Pull a fresh passive connectivity snapshot.
pub async fn read_connectivity_snapshot() -> Result<ConnectivitySnapshot, String> {
    let default_route_ifaces = _read_default_route_ifaces().await?;
    let interface_states = _read_interface_states().await?;
    let dns_state = _read_dns_state().await?;
    let primary_iface = _pick_primary_iface(&default_route_ifaces, &interface_states);

    Ok(ConnectivitySnapshot {
        primary_iface,
        has_default_route: ! default_route_ifaces.is_empty(),
        dns_state,
    })
}

/// Read the interface that best represents current traffic routing.
pub async fn read_primary_interface() -> Result<Option<InterfaceState>, String> {
    let default_route_ifaces = _read_default_route_ifaces().await?;
    let interface_states = _read_interface_states().await?;

    Ok(_pick_primary_iface(&default_route_ifaces, &interface_states))
}

/// Read byte counters for all visible interfaces.
pub async fn read_dev_stats() -> Result<HashMap<String, DevStats>, String> {
    let dev = _read_text("/proc/net/dev").await?;
    let mut stats = HashMap::new();

    for line in dev.split('\n') {
        if ! line.contains(':') {
            continue;
        }

        let line = line.split_once(':').unwrap();
        let iface = line.0.trim().to_string();
        let mut cols = line.1.split_whitespace();
        let recv_bytes = _parse_u128(cols.next());
        let trans_bytes = _parse_u128(cols.nth(7));

        stats.insert(iface, DevStats {
            recv_bytes,
            trans_bytes,
        });
    }

    Ok(stats)
}

/// Classify an interface from its kernel-visible traits.
pub fn interface_kind(iface: &str) -> InterfaceKind {
    if iface == "lo" {
        return InterfaceKind::Other;
    }

    if Path::new(&format!("/sys/class/net/{iface}/wireless")).exists() {
        return InterfaceKind::Wireless;
    }

    if _is_tunnel_iface(iface) {
        return InterfaceKind::Tunnel;
    }

    InterfaceKind::Wired
}

/// Select the multicast groups that matter for passive network state.
fn _connectivity_groups() -> [MulticastGroup; 5] {
    [
        MulticastGroup::Link,
        MulticastGroup::Ipv4Ifaddr,
        MulticastGroup::Ipv6Ifaddr,
        MulticastGroup::Ipv4Route,
        MulticastGroup::Ipv6Route,
    ]
}

/// Keep the netlink connection alive for route and link updates.
async fn _run_connectivity_monitor<F>(connection: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    connection.await;
}

/// Turn raw netlink traffic into coalesced wakeups.
async fn _forward_connectivity_events<S, T>(
    mut messages: S,
    notify: Arc<Notify>,
) where
    S: Stream<Item = T> + Unpin + Send + 'static,
    T: Send + 'static,
{
    while messages.next().await.is_some() {
        notify.notify_one();
    }
}

/// Gather default-route interfaces across IPv4 and IPv6.
async fn _read_default_route_ifaces() -> Result<Vec<String>, String> {
    let mut ifaces = _read_default_route_ifaces_v4().await?;
    let mut v6_ifaces = _read_default_route_ifaces_v6().await?;

    ifaces.append(&mut v6_ifaces);
    ifaces.sort();
    ifaces.dedup();

    Ok(ifaces)
}

/// Read default-route interfaces from the IPv4 route table.
async fn _read_default_route_ifaces_v4() -> Result<Vec<String>, String> {
    let routes = _read_text("/proc/net/route").await?;
    let mut ifaces = vec![];

    for line in routes.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();

        if cols.len() < 2 {
            continue;
        }

        if cols[1] == "00000000" {
            ifaces.push(cols[0].to_string());
        }
    }

    Ok(ifaces)
}

/// Read default-route interfaces from the IPv6 route table.
async fn _read_default_route_ifaces_v6() -> Result<Vec<String>, String> {
    let routes = match _read_text("/proc/net/ipv6_route").await {
        Ok(routes) => routes,
        Err(_) => return Ok(vec![]),
    };
    let mut ifaces = vec![];

    for line in routes.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();

        if cols.len() < 10 {
            continue;
        }

        if cols[0] == "00000000000000000000000000000000" && cols[1] == "00000000" {
            ifaces.push(cols[9].to_string());
        }
    }

    Ok(ifaces)
}

/// Read link state for all interfaces exposed by the kernel.
async fn _read_interface_states() -> Result<HashMap<String, InterfaceState>, String> {
    let dev_stats = read_dev_stats().await?;
    let mut states = HashMap::new();
    let mut ifaces = _read_iface_names()?;

    ifaces.sort();

    for iface in ifaces {
        if ! dev_stats.contains_key(&iface) {
            continue;
        }

        let operstate = read_to_string(format!("/sys/class/net/{iface}/operstate")).await
            .unwrap_or_default();
        let operstate = operstate.trim().to_string();
        let link_up = _read_link_state(&iface, &operstate).await;

        states.insert(iface.clone(), InterfaceState {
            name: iface.clone(),
            kind: interface_kind(&iface),
            link_up,
        });
    }

    Ok(states)
}

/// List interface names from sysfs without parsing command output.
fn _read_iface_names() -> Result<Vec<String>, String> {
    let mut ifaces = vec![];
    let entries = match read_dir("/sys/class/net") {
        Ok(entries) => entries,
        Err(err) => return Err(format!("Failed to read /sys/class/net: {err}")),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let iface = match entry.file_name().into_string() {
            Ok(iface) => iface,
            Err(_) => continue,
        };

        ifaces.push(iface);
    }

    Ok(ifaces)
}

/// Summarize how the resolver is configured locally.
async fn _read_dns_state() -> Result<DnsState, String> {
    let resolv_conf = _read_text("/etc/resolv.conf").await?;
    let mut has_loopback = false;
    let mut has_remote = false;

    for line in resolv_conf.lines() {
        if ! line.trim().starts_with("nameserver") {
            continue;
        }

        let server = match line.split_whitespace().last() {
            Some(server) => server,
            None => continue,
        };

        match server.parse::<IpAddr>() {
            Ok(server) if server.is_loopback() => has_loopback = true,
            Ok(_) => has_remote = true,
            Err(_) => {}
        }
    }

    let dns_state = match (has_loopback, has_remote) {
        (false, false) => DnsState::Missing,
        (true, false) => DnsState::Stub,
        (false, true) => DnsState::Direct,
        (true, true) => DnsState::Mixed,
    };

    Ok(dns_state)
}

/// Prefer carrier when available for a stricter link answer.
async fn _read_link_state(iface: &str, operstate: &str) -> bool {
    let carrier_path = format!("/sys/class/net/{iface}/carrier");

    if Path::new(&carrier_path).exists() {
        let carrier = read_to_string(&carrier_path).await.unwrap_or_default();
        return carrier.trim() == "1";
    }

    matches!(operstate, "up" | "unknown")
}

/// Parse counter fields without exposing iterator plumbing.
fn _parse_u128(value: Option<&str>) -> u128 {
    match value {
        Some(value) => value.parse::<u128>().unwrap_or(0),
        None => 0,
    }
}

/// Read a kernel text file into memory.
async fn _read_text(path: &str) -> Result<String, String> {
    match read_to_string(path).await {
        Ok(contents) => Ok(contents),
        Err(err) => Err(format!("Failed to read {path}: {err}")),
    }
}

/// Pick the interface that best represents current traffic routing.
fn _pick_primary_iface(
    default_route_ifaces: &[String],
    interface_states: &HashMap<String, InterfaceState>,
) -> Option<InterfaceState> {
    for iface in default_route_ifaces {
        if let Some(state) = interface_states.get(iface) {
            return Some(state.clone());
        }
    }

    let mut ifaces = vec![];

    for iface in interface_states.values() {
        if iface.name == "lo" || ! iface.link_up {
            continue;
        }

        ifaces.push(iface.clone());
    }

    ifaces.sort_by_key(_iface_sort_key);

    ifaces.into_iter().next()
}

/// Sort interface candidates by stable name order.
fn _iface_sort_key(iface: &InterfaceState) -> String {
    iface.name.clone()
}

/// Match common tunnel prefixes without provider coupling.
fn _is_tunnel_iface(iface: &str) -> bool {
    iface.starts_with("wg")
        || iface.starts_with("tun")
        || iface.starts_with("tap")
        || iface.starts_with("ppp")
        || iface.starts_with("tailscale")
        || iface.starts_with("zt")
}
