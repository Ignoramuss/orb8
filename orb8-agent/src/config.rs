use log::info;
use std::time::Duration;

pub struct AgentConfig {
    pub grpc_port: u16,
    pub health_port: u16,
    pub max_flows: usize,
    pub flow_timeout: Duration,
    pub max_pod_cache_entries: usize,
    pub broadcast_channel_size: usize,
    pub poll_interval: Duration,
    pub max_batch_size: usize,
    pub shutdown_timeout: Duration,
    pub expiration_interval: Duration,
    pub max_query_limit: usize,
}

impl AgentConfig {
    pub fn from_env() -> Self {
        Self {
            grpc_port: parse_env("ORB8_GRPC_PORT", 9090),
            health_port: parse_env("ORB8_HEALTH_PORT", 9091),
            max_flows: parse_env("ORB8_MAX_FLOWS", 100_000),
            flow_timeout: Duration::from_secs(parse_env("ORB8_FLOW_TIMEOUT_SECS", 30)),
            max_pod_cache_entries: parse_env("ORB8_MAX_POD_CACHE", 10_000),
            broadcast_channel_size: parse_env("ORB8_BROADCAST_CHANNEL_SIZE", 1_000),
            poll_interval: Duration::from_millis(parse_env("ORB8_POLL_INTERVAL_MS", 100)),
            max_batch_size: parse_env("ORB8_MAX_BATCH_SIZE", 1_024),
            shutdown_timeout: Duration::from_secs(parse_env("ORB8_SHUTDOWN_TIMEOUT_SECS", 10)),
            expiration_interval: Duration::from_secs(parse_env(
                "ORB8_EXPIRATION_INTERVAL_SECS",
                10,
            )),
            max_query_limit: parse_env("ORB8_MAX_QUERY_LIMIT", 10_000),
        }
    }

    pub fn log_config(&self) {
        info!("Agent configuration:");
        info!("  gRPC port: {}", self.grpc_port);
        info!("  Health port: {}", self.health_port);
        info!("  Max flows: {}", self.max_flows);
        info!("  Flow timeout: {:?}", self.flow_timeout);
        info!("  Max pod cache entries: {}", self.max_pod_cache_entries);
        info!("  Broadcast channel size: {}", self.broadcast_channel_size);
        info!("  Poll interval: {:?}", self.poll_interval);
        info!("  Max batch size: {}", self.max_batch_size);
        info!("  Shutdown timeout: {:?}", self.shutdown_timeout);
        info!("  Expiration interval: {:?}", self.expiration_interval);
        info!("  Max query limit: {}", self.max_query_limit);
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            grpc_port: 9090,
            health_port: 9091,
            max_flows: 100_000,
            flow_timeout: Duration::from_secs(30),
            max_pod_cache_entries: 10_000,
            broadcast_channel_size: 1_000,
            poll_interval: Duration::from_millis(100),
            max_batch_size: 1_024,
            shutdown_timeout: Duration::from_secs(10),
            expiration_interval: Duration::from_secs(10),
            max_query_limit: 10_000,
        }
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    match std::env::var(key) {
        Ok(val) => match val.parse::<T>() {
            Ok(parsed) => {
                info!("Config override: {}={}", key, val);
                parsed
            }
            Err(_) => {
                log::warn!("Invalid value for {}: '{}', using default", key, val);
                default
            }
        },
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let config = AgentConfig::default();
        assert_eq!(config.grpc_port, 9090);
        assert_eq!(config.health_port, 9091);
        assert_eq!(config.max_flows, 100_000);
        assert_eq!(config.flow_timeout, Duration::from_secs(30));
        assert_eq!(config.max_pod_cache_entries, 10_000);
        assert_eq!(config.broadcast_channel_size, 1_000);
        assert_eq!(config.poll_interval, Duration::from_millis(100));
        assert_eq!(config.max_batch_size, 1_024);
        assert_eq!(config.shutdown_timeout, Duration::from_secs(10));
        assert_eq!(config.expiration_interval, Duration::from_secs(10));
        assert_eq!(config.max_query_limit, 10_000);
    }

    #[test]
    fn test_from_env_uses_defaults_when_unset() {
        let config = AgentConfig::from_env();
        assert_eq!(config.grpc_port, 9090);
        assert_eq!(config.max_flows, 100_000);
    }

    #[test]
    fn test_parse_env_with_invalid_value() {
        std::env::set_var("ORB8_TEST_PARSE", "not_a_number");
        let result: u16 = parse_env("ORB8_TEST_PARSE", 42);
        assert_eq!(result, 42);
        std::env::remove_var("ORB8_TEST_PARSE");
    }

    #[test]
    fn test_parse_env_with_valid_value() {
        std::env::set_var("ORB8_TEST_VALID", "8080");
        let result: u16 = parse_env("ORB8_TEST_VALID", 9090);
        assert_eq!(result, 8080);
        std::env::remove_var("ORB8_TEST_VALID");
    }

    #[test]
    fn test_log_config_does_not_panic() {
        let config = AgentConfig::default();
        config.log_config();
    }
}
