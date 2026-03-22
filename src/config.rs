use serde::Deserialize;

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub features: Vec<String>,
    pub connectivity: ConnectivityConfig,
    pub date_time: DateTimeConfig,
    pub memory: MemoryConfig,
    pub cpu: CpuConfig,
    pub gpu: GpuConfig,
    pub net_stats: NetStatsConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ConnectivityConfig {
    pub prefix: String,
    pub idle: u64,
    pub format: String,
    pub show_iface: bool,
    pub show_route: bool,
    pub show_dns: bool,
    pub show_kind: bool,
}

impl Default for ConnectivityConfig {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            idle: 1,
            format: "compact".to_string(),
            show_iface: true,
            show_route: true,
            show_dns: true,
            show_kind: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DateTimeConfig {
    pub prefix: String,
    pub idle: u64,
    pub format: String,
}

impl Default for DateTimeConfig {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            idle: 1,
            format: "%a %d %b %Y %X %Z".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub prefix: String,
    pub idle: u64,
    pub output: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            idle: 1,
            output: "percentage".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct CpuConfig {
    pub prefix: String,
    pub idle: u64,
    pub chip: String,
    pub report: String,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            idle: 1,
            chip: String::new(),
            report: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct GpuConfig {
    pub prefix: String,
    pub idle: u64,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            idle: 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct NetStatsConfig {
    pub prefix: String,
    pub idle: u64,
    pub ifaces: Vec<String>,
}

impl Default for NetStatsConfig {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            idle: 1,
            ifaces: vec![],
        }
    }
}
