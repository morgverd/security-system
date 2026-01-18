use crate::alerts::AlertLevel;
use anyhow::{Context, Result};
use serde::Deserialize;
use sms_client::config::{ClientConfig, TLSConfig};
use std::collections::HashSet;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub(crate) struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub sentry: SentryConfig,

    #[serde(default)]
    pub alerts: AlertsConfig,

    #[serde(default)]
    pub monitors: MonitorsConfig,

    // REQUIRED
    pub sms: SMSConfig,
}
impl AppConfig {
    pub fn load(config_filepath: Option<PathBuf>) -> Result<Self> {
        let config_path = config_filepath.unwrap_or_else(|| PathBuf::from("config.toml"));

        let config_content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {config_path:?}"))?;

        let config: AppConfig = toml::from_str(&config_content)
            .with_context(|| format!("Failed to parse TOML config file: {config_path:?}"))?;

        Ok(config)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ServerConfig {
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_addr: default_http_addr(),
        }
    }
}

#[derive(Default, Debug, Deserialize)]
pub(crate) struct SentryConfig {
    #[serde(default)]
    pub sentry_dsn: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AlertsConfig {
    #[serde(default = "default_alarm_cooldown")]
    pub alarm_cooldown: u64,

    #[serde(default = "default_alerts_send_retry_max")]
    pub send_retry_max: u64,

    #[serde(default = "default_alerts_send_retry_base_delay")]
    pub send_retry_base_delay: u64,

    #[serde(default = "default_alerts_send_retry_max_delay")]
    pub send_retry_max_delay: u64,

    #[serde(default = "default_alerts_send_concurrency_limit")]
    pub send_concurrency_limit: usize,
}
impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            alarm_cooldown: default_alarm_cooldown(),
            send_retry_max: default_alerts_send_retry_max(),
            send_retry_base_delay: default_alerts_send_retry_base_delay(),
            send_retry_max_delay: default_alerts_send_retry_max_delay(),
            send_concurrency_limit: default_alerts_send_concurrency_limit(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct MonitorsConfig {
    #[serde(default)]
    pub disabled: Option<HashSet<String>>,

    #[serde(default = "default_poll_interval")]
    pub services_poll_interval: u64,

    #[serde(default = "default_poll_interval")]
    pub ping_poll_interval: u64,

    #[serde(default = "default_monitors_ping_poll_timeout")]
    pub ping_poll_timeout: u64,

    #[serde(default)]
    pub cctv_local_ip: Option<String>,

    #[serde(default)]
    pub cron_url: Option<String>,

    #[serde(default = "default_poll_interval")]
    pub cron_interval: u64,
}
impl Default for MonitorsConfig {
    fn default() -> Self {
        Self {
            disabled: None,
            services_poll_interval: default_poll_interval(),
            ping_poll_interval: default_poll_interval(),
            ping_poll_timeout: default_monitors_ping_poll_timeout(),
            cctv_local_ip: None,
            cron_url: None,
            cron_interval: default_poll_interval(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SMSConfig {
    http_base: String,                 // REQUIRED
    pub recipients: Vec<SMSRecipient>, // REQUIRED

    #[serde(default)]
    auth: Option<String>,

    #[serde(default)]
    certificate_path: Option<String>,
}
impl SMSConfig {
    pub fn get_sms_config(&self) -> ClientConfig {
        let mut config = ClientConfig::http_only(&self.http_base);
        if let Some(auth) = &self.auth {
            config = config.with_auth(auth);
        }
        if let Some(certificate_path) = &self.certificate_path {
            config = config.add_tls(
                TLSConfig::new(certificate_path).expect("Invalid SMS certificate filepath!"),
            );
        }
        config
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SMSRecipient {
    pub phone: String, // REQUIRED

    #[serde(default = "default_sms_recipient_level")]
    pub level: u8,
}

fn default_http_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
}
fn default_alarm_cooldown() -> u64 {
    300
}
fn default_alerts_send_retry_max() -> u64 {
    8
}
fn default_alerts_send_retry_base_delay() -> u64 {
    2
}
fn default_alerts_send_retry_max_delay() -> u64 {
    90
}
fn default_alerts_send_concurrency_limit() -> usize {
    4
}
fn default_poll_interval() -> u64 {
    60
}
fn default_monitors_ping_poll_timeout() -> u64 {
    10
}
fn default_sms_recipient_level() -> u8 {
    AlertLevel::Alarm.as_u8()
}
