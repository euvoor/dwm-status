use std::time::Duration;
use serde::Deserialize;

#[derive(Clone, Default, Debug, Deserialize)]
pub struct Config {
    pub features: Vec<String>,
    pub vpn: VpnConfig,
    pub date_time: DateTimeConfig,
    pub ping: PingConfig,
    pub memory: MemoryConfig,
    pub cpu: CpuConfig,
    pub gpu: GpuConfig,
    pub net_stats: NetStatsConfig,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct VpnConfig {
    pub prefix: String,
    pub idle: u64,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct DateTimeConfig {
    pub prefix: String,
    pub idle: u64,
    pub format: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct PingConfig {
    pub prefix: String,
    pub idle: u64,
    pub no_internet: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct MemoryConfig {
    pub prefix: String,
    pub idle: u64,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct CpuConfig {
    pub prefix: String,
    pub idle: u64,
    pub chip: String,
    pub report: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct GpuConfig {
    pub prefix: String,
    pub idle: u64,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct NetStatsConfig {
    pub prefix: String,
    pub idle: u64,
    pub ifaces: Vec<String>,
}
