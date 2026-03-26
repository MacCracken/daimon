//! Configuration for the daimon service.

use serde::{Deserialize, Serialize};

/// Service configuration.
///
/// # Examples
///
/// ```
/// # use daimon::Config;
/// let cfg = Config::default();
/// assert_eq!(cfg.port, 8090);
/// assert_eq!(cfg.listen_addr, "127.0.0.1");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Config {
    /// Listen address for the HTTP API.
    pub listen_addr: String,

    /// Port for the HTTP API.
    pub port: u16,

    /// Data directory for persistent storage.
    pub data_dir: String,

    /// Maximum number of agents that can be registered.
    pub max_agents: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1".to_string(),
            port: 8090,
            data_dir: "/var/lib/agnos".to_string(),
            max_agents: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.listen_addr, "127.0.0.1");
        assert_eq!(cfg.port, 8090);
        assert_eq!(cfg.data_dir, "/var/lib/agnos");
        assert_eq!(cfg.max_agents, 1000);
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = Config::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.port, cfg.port);
        assert_eq!(deserialized.listen_addr, cfg.listen_addr);
        assert_eq!(deserialized.data_dir, cfg.data_dir);
        assert_eq!(deserialized.max_agents, cfg.max_agents);
    }
}
