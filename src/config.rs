use serde::Deserialize;

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub features: Vec<String>,
    pub connectivity: ConnectivityConfig,
    pub clock: ClockConfig,
    pub ram: RamConfig,
    pub cpu: CpuConfig,
    pub gpu: GpuConfig,
    pub traffic: TrafficConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ConnectivityConfig {
    pub glyph: String,
    pub idle: u64,
    pub format: String,
    pub show_iface: bool,
    pub show_route: bool,
    pub show_dns: bool,
    pub show_kind: bool,
}

impl Default for ConnectivityConfig {
    /// Default passive connectivity settings.
    fn default() -> Self {
        Self {
            glyph: String::new(),
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
pub struct ClockConfig {
    pub glyph: String,
    pub format: String,
    pub timezone: String,
}

impl Default for ClockConfig {
    /// Default clock formatting.
    fn default() -> Self {
        Self {
            glyph: String::new(),
            format: "%a %d %b %Y %X %Z".to_string(),
            timezone: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RamConfig {
    pub glyph: String,
}

impl Default for RamConfig {
    /// Default RAM reporting settings.
    fn default() -> Self {
        Self {
            glyph: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct CpuConfig {
    pub glyph: String,
    pub sparkline_width: usize,
}

impl Default for CpuConfig {
    /// Default CPU reporting settings.
    fn default() -> Self {
        Self {
            glyph: String::new(),
            sparkline_width: 8,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct GpuConfig {
    pub glyph: String,
    pub idle: u64,
}

impl Default for GpuConfig {
    /// Default GPU reporting settings.
    fn default() -> Self {
        Self {
            glyph: String::new(),
            idle: 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct TrafficConfig {
    pub glyph: String,
}

impl Default for TrafficConfig {
    /// Default traffic reporting settings.
    fn default() -> Self {
        Self {
            glyph: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    /// Parse a named timezone from clock config.
    #[test]
    fn parse_clock_config() {
        let config = toml::from_str::<Config>(
            r#"
features = ["clock"]

[clock]
glyph = "clk "
format = "%H:%M"
timezone = "Europe/Berlin"
"#,
        ).unwrap();

        assert_eq!(config.features, vec!["clock"]);
        assert_eq!(config.clock.glyph, "clk ");
        assert_eq!(config.clock.format, "%H:%M");
        assert_eq!(config.clock.timezone, "Europe/Berlin");
    }

    /// Default an empty timezone string to local machine time.
    #[test]
    fn default_clock_timezone_is_empty() {
        let config = toml::from_str::<Config>(
            r#"
features = ["clock"]

[clock]
glyph = "clk "
format = "%H:%M"
"#,
        ).unwrap();

        assert_eq!(config.clock.timezone, "");
    }

    /// Parse the compact CPU config without legacy knobs.
    #[test]
    fn parse_cpu_config() {
        let config = toml::from_str::<Config>(
            r#"
features = ["cpu"]

[cpu]
glyph = "cpu "
sparkline_width = 0
"#,
        ).unwrap();

        assert_eq!(config.features, vec!["cpu"]);
        assert_eq!(config.cpu.glyph, "cpu ");
        assert_eq!(config.cpu.sparkline_width, 0);
    }

    /// Parse the compact traffic config without interface lists.
    #[test]
    fn parse_traffic_config() {
        let config = toml::from_str::<Config>(
            r#"
features = ["traffic"]

[traffic]
glyph = "net "
"#,
        ).unwrap();

        assert_eq!(config.features, vec!["traffic"]);
        assert_eq!(config.traffic.glyph, "net ");
    }
}
