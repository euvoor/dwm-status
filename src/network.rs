use std::collections::HashMap;
use std::fs::read_dir;
use std::net::IpAddr;
use std::path::Path;

use tokio::fs::read_to_string;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsState {
    Missing,
    Stub,
    Direct,
    Mixed,
}

impl DnsState {
    pub fn compact_label(&self) -> &'static str {
        match self {
            Self::Missing => "no-dns",
            Self::Stub => "stub",
            Self::Direct => "dns",
            Self::Mixed => "mixed",
        }
    }

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
    pub fn compact_label(&self) -> &'static str {
        match self {
            Self::Wireless => "W",
            Self::Wired => "E",
            Self::Tunnel => "T",
            Self::Other => "N",
        }
    }

    pub fn full_label(&self) -> &'static str {
        match self {
            Self::Wireless => "wifi",
            Self::Wired => "ethernet",
            Self::Tunnel => "tunnel",
            Self::Other => "other",
        }
    }

    pub fn net_stats_label(&self) -> &'static str {
        match self {
            Self::Wireless => "W",
            Self::Wired => "E",
            Self::Tunnel => "T",
            Self::Other => "N",
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

pub async fn read_dev_stats() -> Result<HashMap<String, DevStats>, String> {
    let dev = read_to_string("/proc/net/dev").await
        .map_err(|err| format!("Failed to read /proc/net/dev: {err}"))?;
    let mut stats = HashMap::new();

    dev.split('\n').for_each(|line| {
        if ! line.contains(':') {
            return;
        }

        let line = line.split_once(':').unwrap();
        let iface = line.0.trim().to_string();
        let mut cols = line.1.split_whitespace();
        let recv_bytes = cols.next()
            .and_then(|value| value.parse::<u128>().ok())
            .unwrap_or(0);
        let trans_bytes = cols.nth(7)
            .and_then(|value| value.parse::<u128>().ok())
            .unwrap_or(0);

        stats.insert(iface, DevStats {
            recv_bytes,
            trans_bytes,
        });
    });

    Ok(stats)
}

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

async fn _read_default_route_ifaces() -> Result<Vec<String>, String> {
    let mut ifaces = _read_default_route_ifaces_v4().await?;
    let mut v6_ifaces = _read_default_route_ifaces_v6().await?;

    ifaces.append(&mut v6_ifaces);
    ifaces.sort();
    ifaces.dedup();

    Ok(ifaces)
}

async fn _read_default_route_ifaces_v4() -> Result<Vec<String>, String> {
    let routes = read_to_string("/proc/net/route").await
        .map_err(|err| format!("Failed to read /proc/net/route: {err}"))?;
    let mut ifaces = vec![];

    routes.lines().skip(1).for_each(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();

        if cols.len() < 2 {
            return;
        }

        if cols[1] == "00000000" {
            ifaces.push(cols[0].to_string());
        }
    });

    Ok(ifaces)
}

async fn _read_default_route_ifaces_v6() -> Result<Vec<String>, String> {
    let routes = match read_to_string("/proc/net/ipv6_route").await {
        Ok(routes) => routes,
        Err(_) => return Ok(vec![]),
    };
    let mut ifaces = vec![];

    routes.lines().for_each(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();

        if cols.len() < 10 {
            return;
        }

        if cols[0] == "00000000000000000000000000000000" && cols[1] == "00000000" {
            ifaces.push(cols[9].to_string());
        }
    });

    Ok(ifaces)
}

async fn _read_interface_states() -> Result<HashMap<String, InterfaceState>, String> {
    let dev_stats = read_dev_stats().await?;
    let mut states = HashMap::new();

    let mut ifaces = read_dir("/sys/class/net")
        .map_err(|err| format!("Failed to read /sys/class/net: {err}"))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<String>>();

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

async fn _read_dns_state() -> Result<DnsState, String> {
    let resolv_conf = match read_to_string("/etc/resolv.conf").await {
        Ok(resolv_conf) => resolv_conf,
        Err(err) => return Err(format!("Failed to read /etc/resolv.conf: {err}")),
    };
    let mut has_loopback = false;
    let mut has_remote = false;

    resolv_conf.lines().for_each(|line| {
        if ! line.trim().starts_with("nameserver") {
            return;
        }

        let server = match line.split_whitespace().last() {
            Some(server) => server,
            None => return,
        };

        match server.parse::<IpAddr>() {
            Ok(server) if server.is_loopback() => has_loopback = true,
            Ok(_) => has_remote = true,
            Err(_) => {}
        }
    });

    let dns_state = match (has_loopback, has_remote) {
        (false, false) => DnsState::Missing,
        (true, false) => DnsState::Stub,
        (false, true) => DnsState::Direct,
        (true, true) => DnsState::Mixed,
    };

    Ok(dns_state)
}

async fn _read_link_state(iface: &str, operstate: &str) -> bool {
    let carrier_path = format!("/sys/class/net/{iface}/carrier");

    if Path::new(&carrier_path).exists() {
        let carrier = read_to_string(&carrier_path).await.unwrap_or_default();
        return carrier.trim() == "1";
    }

    matches!(operstate, "up" | "unknown")
}

fn _pick_primary_iface(
    default_route_ifaces: &Vec<String>,
    interface_states: &HashMap<String, InterfaceState>,
) -> Option<InterfaceState> {
    for iface in default_route_ifaces {
        if let Some(state) = interface_states.get(iface) {
            return Some(state.clone());
        }
    }

    let mut ifaces = interface_states.values()
        .filter(|iface| iface.name != "lo" && iface.link_up)
        .cloned()
        .collect::<Vec<InterfaceState>>();

    ifaces.sort_by(|left, right| left.name.cmp(&right.name));

    ifaces.into_iter().next()
}

fn _is_tunnel_iface(iface: &str) -> bool {
    iface.starts_with("wg")
        || iface.starts_with("tun")
        || iface.starts_with("tap")
        || iface.starts_with("ppp")
        || iface.starts_with("tailscale")
        || iface.starts_with("zt")
}
