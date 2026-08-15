use serde::Deserialize;

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub features: Vec<FeatureName>,
    pub connectivity: ConnectivityConfig,
    pub clock: ClockConfig,
    pub ram: RamConfig,
    pub cpu: CpuConfig,
    pub gpu: GpuConfig,
    pub traffic: TrafficConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FeatureName {
    Connectivity,
    Traffic,
    Cpu,
    Clock,
    Ram,
    Gpu,
}

impl FeatureName {
    /// Return the spelling accepted in TOML.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connectivity => "connectivity",
            Self::Traffic => "traffic",
            Self::Cpu => "cpu",
            Self::Clock => "clock",
            Self::Ram => "ram",
            Self::Gpu => "gpu",
        }
    }
}

impl Config {
    /// Reject settings that deserialize but cannot run safely.
    pub fn validate(&self) -> Result<(), String> {
        if self.features.is_empty() {
            return Err("At least one feature is required".to_string());
        }

        for (index, feature) in self.features.iter().enumerate() {
            if self.features[..index].contains(feature) {
                return Err(format!("Duplicate feature: {}", feature.as_str()));
            }
        }

        if self.connectivity.idle == 0 {
            return Err("connectivity.idle must be greater than zero".to_string());
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConnectivityConfig {
    pub glyph: String,
    pub idle: u64,
    pub format: ConnectivityFormat,
    pub show_iface: bool,
    pub show_route: bool,
    pub show_dns: bool,
    pub show_kind: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectivityFormat {
    Compact,
    Full,
}

impl Default for ConnectivityConfig {
    /// Default passive connectivity settings.
    fn default() -> Self {
        Self {
            glyph: String::new(),
            idle: 1,
            format: ConnectivityFormat::Compact,
            show_iface: true,
            show_route: true,
            show_dns: true,
            show_kind: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
#[serde(default, deny_unknown_fields)]
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
#[serde(default, deny_unknown_fields)]
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
#[serde(default, deny_unknown_fields)]
pub struct GpuConfig {
    pub glyph: String,
}

impl Default for GpuConfig {
    /// Default GPU reporting settings.
    fn default() -> Self {
        Self {
            glyph: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
    use super::{Config, ConnectivityFormat, FeatureName};

    /// Keep the checked-in sample config parseable.
    #[test]
    fn parse_sample_config() {
        let config = toml::from_str::<Config>(include_str!("../config.toml")).unwrap();

        assert_eq!(config.features.len(), 6);
        assert_eq!(config.clock.timezone, "");
        assert_eq!(config.validate(), Ok(()));
    }

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

        assert_eq!(config.features, vec![FeatureName::Clock]);
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

        assert_eq!(config.features, vec![FeatureName::Cpu]);
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

        assert_eq!(config.features, vec![FeatureName::Traffic]);
        assert_eq!(config.traffic.glyph, "net ");
    }

    /// Parse the compact GPU config without a timing knob.
    #[test]
    fn parse_gpu_config() {
        let config = toml::from_str::<Config>(
            r#"
features = ["gpu"]

[gpu]
glyph = "gpu "
"#,
        ).unwrap();

        assert_eq!(config.features, vec![FeatureName::Gpu]);
        assert_eq!(config.gpu.glyph, "gpu ");
    }

    /// Reject a misspelled top-level key with its original spelling.
    #[test]
    fn reject_unknown_top_level_key() {
        let error = toml::from_str::<Config>(
            r#"
features = ["clock"]
featurs = ["cpu"]
"#,
        ).unwrap_err();

        assert!(error.to_string().contains("featurs"));
    }

    /// Reject a misspelled feature-table key with its original spelling.
    #[test]
    fn reject_unknown_nested_key() {
        let error = toml::from_str::<Config>(
            r#"
features = ["cpu"]

[cpu]
sparkline_wdith = 4
"#,
        ).unwrap_err();

        assert!(error.to_string().contains("sparkline_wdith"));
    }

    /// Keep connectivity layouts limited to the documented values.
    #[test]
    fn reject_invalid_connectivity_format() {
        let error = toml::from_str::<Config>(
            r#"
features = ["connectivity"]

[connectivity]
format = "wide"
"#,
        ).unwrap_err();

        assert!(error.to_string().contains("wide"));
        assert!(error.to_string().contains("compact"));
        assert!(error.to_string().contains("full"));
    }

    /// Reject duplicate workers before runtime startup.
    #[test]
    fn reject_duplicate_features() {
        let config = toml::from_str::<Config>(
            r#"features = ["clock", "cpu", "clock"]"#,
        ).unwrap();

        assert_eq!(
            config.validate(),
            Err("Duplicate feature: clock".to_string()),
        );
    }

    /// Refuse a status process that can never render a feature.
    #[test]
    fn reject_empty_feature_list() {
        let config = toml::from_str::<Config>("features = []").unwrap();

        assert_eq!(
            config.validate(),
            Err("At least one feature is required".to_string()),
        );
    }

    /// Refuse a zero resync interval instead of rewriting it.
    #[test]
    fn reject_zero_connectivity_idle() {
        let config = toml::from_str::<Config>(
            r#"
features = ["connectivity"]

[connectivity]
idle = 0
"#,
        ).unwrap();

        assert_eq!(
            config.validate(),
            Err("connectivity.idle must be greater than zero".to_string()),
        );
    }

    /// Retain defaults when a valid feature table is only partial.
    #[test]
    fn default_valid_partial_table() {
        let config = toml::from_str::<Config>(
            r#"
features = ["connectivity"]

[connectivity]
glyph = "net "
"#,
        ).unwrap();

        assert_eq!(config.connectivity.glyph, "net ");
        assert_eq!(config.connectivity.idle, 1);
        assert_eq!(config.connectivity.format, ConnectivityFormat::Compact);
        assert!(config.connectivity.show_iface);
        assert_eq!(config.validate(), Ok(()));
    }
}
