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

const ROUTE_FLAG_UP: u32 = 0x0001;
const ROUTE_FLAG_REJECT: u32 = 0x0200;

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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RouteFamily {
    Ipv4,
    Ipv6,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DefaultRoute {
    iface: String,
    metric: u32,
    flags: u32,
    family: RouteFamily,
}

struct PrimarySelection {
    primary_iface: Option<InterfaceState>,
    has_default_route: bool,
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
    let default_routes = _read_default_routes().await?;
    let interface_states = _read_interface_states().await?;
    let dns_state = _read_dns_state().await?;
    let selection = _select_primary_iface(&default_routes, &interface_states);

    Ok(ConnectivitySnapshot {
        primary_iface: selection.primary_iface,
        has_default_route: selection.has_default_route,
        dns_state,
    })
}

/// Read the interface that best represents current traffic routing.
pub async fn read_primary_interface() -> Result<Option<InterfaceState>, String> {
    let default_routes = _read_default_routes().await?;
    let interface_states = _read_interface_states().await?;

    Ok(_select_primary_iface(&default_routes, &interface_states).primary_iface)
}

/// Read byte counters for all visible interfaces.
pub async fn read_dev_stats() -> Result<HashMap<String, DevStats>, String> {
    let dev = _read_text("/proc/net/dev").await?;

    Ok(_from_dev_stats(dev.as_str()))
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

/// Gather typed default routes across IPv4 and IPv6.
async fn _read_default_routes() -> Result<Vec<DefaultRoute>, String> {
    let mut routes = _read_default_routes_v4().await?;
    let mut v6_routes = _read_default_routes_v6().await?;

    routes.append(&mut v6_routes);

    Ok(routes)
}

/// Read typed defaults from the IPv4 route table.
async fn _read_default_routes_v4() -> Result<Vec<DefaultRoute>, String> {
    let routes = _read_text("/proc/net/route").await?;

    Ok(_from_default_routes_v4(routes.as_str()))
}

/// Parse usable IPv4 defaults from procfs text.
fn _from_default_routes_v4(routes: &str) -> Vec<DefaultRoute> {
    let mut defaults = vec![];

    for line in routes.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();

        if cols.len() < 8
            || ! _is_zero_hex(cols[1], 8)
            || ! _is_zero_hex(cols[7], 8)
        {
            continue;
        }

        let flags = match _from_hex_u32(cols[3]) {
            Some(flags) => flags,
            None => continue,
        };
        let metric = match cols[6].parse::<u32>() {
            Ok(metric) => metric,
            Err(_) => continue,
        };

        defaults.push(DefaultRoute {
            iface: cols[0].to_string(),
            metric,
            flags,
            family: RouteFamily::Ipv4,
        });
    }

    defaults
}

/// Read typed defaults from the IPv6 route table.
async fn _read_default_routes_v6() -> Result<Vec<DefaultRoute>, String> {
    let routes = match _read_text("/proc/net/ipv6_route").await {
        Ok(routes) => routes,
        Err(_) => return Ok(vec![]),
    };

    Ok(_from_default_routes_v6(routes.as_str()))
}

/// Parse usable IPv6 defaults from procfs text.
fn _from_default_routes_v6(routes: &str) -> Vec<DefaultRoute> {
    let mut defaults = vec![];

    for line in routes.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();

        if cols.len() < 10
            || ! _is_zero_hex(cols[0], 32)
            || _from_hex_u32(cols[1]) != Some(0)
            || ! _is_zero_hex(cols[2], 32)
            || _from_hex_u32(cols[3]) != Some(0)
            || cols[4].len() != 32
            || ! cols[4].bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            continue;
        }

        let metric = match _from_hex_u32(cols[5]) {
            Some(metric) => metric,
            None => continue,
        };
        let flags = match _from_hex_u32(cols[8]) {
            Some(flags) => flags,
            None => continue,
        };

        defaults.push(DefaultRoute {
            iface: cols[9].to_string(),
            metric,
            flags,
            family: RouteFamily::Ipv6,
        });
    }

    defaults
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

    Ok(_from_dns_state(resolv_conf.as_str()))
}

/// Parse resolver state from resolv.conf text.
fn _from_dns_state(resolv_conf: &str) -> DnsState {
    let mut has_loopback = false;
    let mut has_remote = false;

    for line in resolv_conf.lines() {
        let mut fields = line.split_whitespace();

        if fields.next() != Some("nameserver") {
            continue;
        }

        let server = match fields.next() {
            Some(server) => server,
            None => continue,
        };

        match server.parse::<IpAddr>() {
            Ok(server) if server.is_loopback() => has_loopback = true,
            Ok(_) => has_remote = true,
            Err(_) => {}
        }
    }

    match (has_loopback, has_remote) {
        (false, false) => DnsState::Missing,
        (true, false) => DnsState::Stub,
        (false, true) => DnsState::Direct,
        (true, true) => DnsState::Mixed,
    }
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

/// Pick the lowest-metric default on an up link, then a stable fallback.
fn _select_primary_iface(
    default_routes: &[DefaultRoute],
    interface_states: &HashMap<String, InterfaceState>,
) -> PrimarySelection {
    let mut best_route: Option<&DefaultRoute> = None;

    for route in default_routes {
        if route.flags & ROUTE_FLAG_UP == 0 || route.flags & ROUTE_FLAG_REJECT != 0 {
            continue;
        }

        let Some(state) = interface_states.get(&route.iface) else {
            continue;
        };

        if ! state.link_up {
            continue;
        }

        match best_route {
            Some(current) if _route_sort_key(current) <= _route_sort_key(route) => {}
            _ => best_route = Some(route),
        }
    }

    if let Some(route) = best_route {
        return PrimarySelection {
            primary_iface: interface_states.get(&route.iface).cloned(),
            has_default_route: true,
        };
    }

    let mut ifaces = vec![];

    for iface in interface_states.values() {
        if iface.name == "lo" || ! iface.link_up {
            continue;
        }

        ifaces.push(iface.clone());
    }

    ifaces.sort_by_key(_iface_sort_key);

    PrimarySelection {
        primary_iface: ifaces.into_iter().next(),
        has_default_route: false,
    }
}

/// Order routes by metric, interface name, then address family.
fn _route_sort_key(route: &DefaultRoute) -> (u32, &str, RouteFamily) {
    (route.metric, route.iface.as_str(), route.family)
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

/// Parse interface counters from procfs text.
fn _from_dev_stats(dev: &str) -> HashMap<String, DevStats> {
    let mut stats = HashMap::new();

    for line in dev.split('\n') {
        if ! line.contains(':') {
            continue;
        }

        let line = match line.split_once(':') {
            Some(line) => line,
            None => continue,
        };
        let iface = line.0.trim().to_string();
        let mut cols = line.1.split_whitespace();
        let recv_bytes = _parse_u128(cols.next());
        let trans_bytes = _parse_u128(cols.nth(7));

        stats.insert(iface, DevStats {
            recv_bytes,
            trans_bytes,
        });
    }

    stats
}

/// Parse a procfs hexadecimal field without accepting signs or prefixes.
fn _from_hex_u32(value: &str) -> Option<u32> {
    u32::from_str_radix(value, 16).ok()
}

/// Validate a fixed-width all-zero hexadecimal field.
fn _is_zero_hex(value: &str, width: usize) -> bool {
    value.len() == width && value.bytes().all(|byte| byte == b'0')
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        _from_default_routes_v4,
        _from_default_routes_v6,
        _from_dev_stats,
        _from_dns_state,
        _route_sort_key,
        _select_primary_iface,
        DefaultRoute,
        DnsState,
        InterfaceKind,
        InterfaceState,
        RouteFamily,
    };

    /// Parse device counters without reading the host network namespace.
    #[test]
    fn parse_dev_stats_fixture() {
        let stats = _from_dev_stats(include_str!("../tests/fixtures/proc/net/dev.txt"));
        let ethernet = stats.get("enp10s0").unwrap();

        assert_eq!(ethernet.recv_bytes, 1_234);
        assert_eq!(ethernet.trans_bytes, 5_678);
    }

    /// Keep malformed procfs counter fields deterministic.
    #[test]
    fn parse_malformed_dev_counter_as_zero() {
        let stats = _from_dev_stats("eth0: nope 0 0 0 0 0 0 0 42 0 0 0 0 0 0 0\n");
        let ethernet = stats.get("eth0").unwrap();

        assert_eq!(ethernet.recv_bytes, 0);
        assert_eq!(ethernet.trans_bytes, 42);
    }

    /// Parse IPv4 default-route interfaces from procfs text.
    #[test]
    fn parse_ipv4_default_route_fixture() {
        let routes = _from_default_routes_v4(
            include_str!("../tests/fixtures/proc/net/route.txt"),
        );

        assert_eq!(routes, vec![DefaultRoute {
            iface: "enp10s0".to_string(),
            metric: 1002,
            flags: 0x0003,
            family: RouteFamily::Ipv4,
        }]);
    }

    /// Parse IPv6 default-route interfaces from procfs text.
    #[test]
    fn parse_ipv6_default_route_fixture() {
        let routes = _from_default_routes_v6(
            include_str!("../tests/fixtures/proc/net/ipv6_route.txt"),
        );

        assert_eq!(routes, vec![DefaultRoute {
            iface: "wlp4s0".to_string(),
            metric: 1024,
            flags: 0x0003,
            family: RouteFamily::Ipv6,
        }]);
    }

    /// Keep IPv4 state flags while rejecting malformed and non-default rows.
    #[test]
    fn filter_ipv4_route_candidates() {
        let routes = _from_default_routes_v4(
            include_str!("../tests/fixtures/proc/net/route_selection_v4.txt"),
        );
        let names: Vec<&str> = routes.iter().map(|route| route.iface.as_str()).collect();

        assert_eq!(names, vec!["z-wan", "a-wan", "down0", "reject0", "inactive0"]);
    }

    /// Keep IPv6 state flags while rejecting malformed and non-default rows.
    #[test]
    fn filter_ipv6_route_candidates() {
        let routes = _from_default_routes_v6(
            include_str!("../tests/fixtures/proc/net/route_selection_v6.txt"),
        );
        let names: Vec<&str> = routes.iter().map(|route| route.iface.as_str()).collect();

        assert_eq!(names, vec!["b-v6", "down6", "reject6", "inactive6"]);
    }

    /// Pick the lowest-metric route only when its link is up.
    #[test]
    fn select_lowest_metric_active_route() {
        let routes = vec![
            DefaultRoute {
                iface: "a-high".to_string(),
                metric: 100,
                flags: 0x0003,
                family: RouteFamily::Ipv4,
            },
            DefaultRoute {
                iface: "z-low".to_string(),
                metric: 10,
                flags: 0x0003,
                family: RouteFamily::Ipv4,
            },
            DefaultRoute {
                iface: "down0".to_string(),
                metric: 1,
                flags: 0x0003,
                family: RouteFamily::Ipv4,
            },
            DefaultRoute {
                iface: "reject0".to_string(),
                metric: 0,
                flags: 0x0201,
                family: RouteFamily::Ipv4,
            },
            DefaultRoute {
                iface: "inactive0".to_string(),
                metric: 0,
                flags: 0x0002,
                family: RouteFamily::Ipv4,
            },
        ];
        let states = _interface_states(&[
            ("a-high", true),
            ("z-low", true),
            ("down0", false),
            ("reject0", true),
            ("inactive0", true),
        ]);
        let selection = _select_primary_iface(&routes, &states);

        assert_eq!(selection.primary_iface.unwrap().name, "z-low");
        assert!(selection.has_default_route);
    }

    /// Break equal route metrics by interface name, then IPv4 before IPv6.
    #[test]
    fn select_route_with_deterministic_family_tie_break() {
        let states = _interface_states(&[("a-v6", true), ("z-v4", true)]);
        let routes = vec![
            DefaultRoute {
                iface: "z-v4".to_string(),
                metric: 50,
                flags: 0x0003,
                family: RouteFamily::Ipv4,
            },
            DefaultRoute {
                iface: "a-v6".to_string(),
                metric: 50,
                flags: 0x0003,
                family: RouteFamily::Ipv6,
            },
        ];
        let selection = _select_primary_iface(&routes, &states);
        let ipv6 = DefaultRoute {
            iface: "same0".to_string(),
            metric: 50,
            flags: 0x0003,
            family: RouteFamily::Ipv6,
        };
        let ipv4 = DefaultRoute {
            iface: "same0".to_string(),
            metric: 50,
            flags: 0x0003,
            family: RouteFamily::Ipv4,
        };

        assert_eq!(selection.primary_iface.unwrap().name, "a-v6");
        assert!(_route_sort_key(&ipv4) < _route_sort_key(&ipv6));
    }

    /// Fall back to the first named up non-loopback link without claiming a route.
    #[test]
    fn fallback_without_usable_default_route() {
        let states = _interface_states(&[("z-wan", true), ("a-wan", true), ("down0", false), ("lo", true)]);
        let selection = _select_primary_iface(&[], &states);

        assert_eq!(selection.primary_iface.unwrap().name, "a-wan");
        assert!(!selection.has_default_route);
    }

    /// Classify resolver state from fixture text.
    #[test]
    fn parse_dns_fixture() {
        let state = _from_dns_state(include_str!("../tests/fixtures/etc/resolv.conf"));

        assert_eq!(state, DnsState::Mixed);
    }

    /// Parse only exact nameserver directives and their second token.
    #[test]
    fn parse_dns_edge_case_fixture() {
        let state = _from_dns_state(include_str!("../tests/fixtures/etc/resolv_edge_cases.conf"));

        assert_eq!(state, DnsState::Mixed);
    }

    /// Distinguish missing, stub, direct, and mixed resolver state.
    #[test]
    fn classify_all_dns_states() {
        let cases = [
            ("# nameserver 127.0.0.1\nnameserver-invalid 1.1.1.1\nnameserver nope 9.9.9.9\n", DnsState::Missing),
            ("nameserver 127.0.0.53\nnameserver ::1\n", DnsState::Stub),
            ("nameserver 1.1.1.1\nnameserver 2606:4700:4700::1111\n", DnsState::Direct),
            ("nameserver 127.0.0.53\nnameserver 9.9.9.9\n", DnsState::Mixed),
        ];

        for (input, expected) in cases {
            assert_eq!(_from_dns_state(input), expected);
        }
    }

    /// Build deterministic interface maps for route selection.
    fn _interface_states(ifaces: &[(&str, bool)]) -> HashMap<String, InterfaceState> {
        let mut states = HashMap::new();

        for (name, link_up) in ifaces {
            states.insert((*name).to_string(), InterfaceState {
                name: (*name).to_string(),
                kind: InterfaceKind::Wired,
                link_up: *link_up,
            });
        }

        states
    }
}
