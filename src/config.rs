use crate::alerts::AlertLevel;
use anyhow::Context;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AppConfig {
    #[serde(default)]
    pub http: HttpConfig,

    #[serde(default)]
    pub sentry: SentryConfig,

    #[serde(default)]
    pub alerts: AlertsConfig,

    #[serde(default)]
    pub monitors: MonitorsConfig,

    #[serde(default)]
    pub communications: CommunicationsConfig,
}
impl AppConfig {
    pub fn load(config_filepath: Option<std::path::PathBuf>) -> anyhow::Result<Self> {
        let config_path =
            config_filepath.unwrap_or_else(|| std::path::PathBuf::from("config.toml"));

        let config_content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {config_path:?}"))?;

        let config: AppConfig = toml::from_str(&config_content)
            .with_context(|| format!("Failed to parse TOML config file: {config_path:?}"))?;

        Ok(config)
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct HttpConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: std::net::SocketAddr,
}
impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
        }
    }
}

#[derive(Default, Debug, serde::Deserialize)]
pub(crate) struct SentryConfig {
    #[serde(default)]
    pub dsn: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AlertsConfig {
    #[serde(default = "default_alarm_cooldown")]
    pub alarm_cooldown: u64,

    #[serde(default = "default_alerts_send_concurrency_limit")]
    pub send_concurrency_limit: usize,
}
impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            alarm_cooldown: default_alarm_cooldown(),
            send_concurrency_limit: default_alerts_send_concurrency_limit(),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct MonitorsConfig {
    #[serde(default)]
    pub disabled: Option<std::collections::HashSet<String>>,

    #[serde(default = "default_poll_interval")]
    pub systemctl_poll_interval: u64,

    #[serde(default = "default_systemctl_retry_attempts")]
    pub systemctl_retry_attempts: u8,

    #[serde(default = "default_systemctl_retry_delay")]
    pub systemctl_retry_delay: u64,

    #[serde(default)]
    pub systemctl: Option<Vec<MonitoredService>>,

    #[serde(default)]
    pub pings: Option<Vec<MonitoredPingTarget>>,

    #[serde(default)]
    pub healthcheck: Option<String>,

    #[serde(default = "default_poll_interval")]
    pub healthcheck_interval: u64,
}
impl Default for MonitorsConfig {
    fn default() -> Self {
        Self {
            disabled: None,
            systemctl_poll_interval: default_poll_interval(),
            systemctl_retry_attempts: default_systemctl_retry_attempts(),
            systemctl_retry_delay: default_systemctl_retry_delay(),
            systemctl: None,
            pings: None,
            healthcheck: None,
            healthcheck_interval: default_poll_interval(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct MonitoredService {
    pub name: String,
    pub level: u8,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct MonitoredPingTarget {
    pub name: String,
    pub addr: String,
    pub level: u8,

    #[serde(default)]
    pub timeout: Option<u64>,

    #[serde(default)]
    pub interval: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct CommunicationsConfig {
    #[serde(default)]
    pub pushover: Option<PushoverCommunicationConfig>,

    #[serde(default)]
    pub sms: Option<SMSCommunicationConfig>,

    #[serde(default = "default_communications_retry_max")]
    pub retry_max: u64,

    #[serde(default = "default_communications_retry_delay")]
    pub retry_delay: u64,
}
impl Default for CommunicationsConfig {
    fn default() -> Self {
        Self {
            pushover: None,
            sms: None,
            retry_max: default_communications_retry_max(),
            retry_delay: default_communications_retry_delay(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct CommunicationRecipient {
    pub target: String,

    #[serde(default = "default_sms_recipient_level")]
    pub level: u8,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct PushoverCommunicationConfig {
    pub token: String,                           // REQUIRED
    pub recipients: Vec<CommunicationRecipient>, // REQUIRED

    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct SMSCommunicationConfig {
    http_base: String,                           // REQUIRED
    pub recipients: Vec<CommunicationRecipient>, // REQUIRED

    #[serde(default)]
    auth: Option<String>,

    #[serde(default)]
    certificate_path: Option<String>,
}
impl SMSCommunicationConfig {
    pub fn get_sms_config(&self) -> sms_client::config::ClientConfig {
        let mut config = sms_client::config::ClientConfig::http_only(&self.http_base);
        if let Some(auth) = &self.auth {
            config = config.with_auth(auth);
        }
        if let Some(certificate_path) = &self.certificate_path {
            config = config.add_tls(
                sms_client::config::TLSConfig::new(certificate_path)
                    .expect("Invalid SMS certificate filepath!"),
            );
        }
        config
    }
}

fn default_poll_interval() -> u64 {
    60
}
fn default_timeout() -> u64 {
    10
}
fn default_bind_address() -> std::net::SocketAddr {
    std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        8080,
    )
}
fn default_alarm_cooldown() -> u64 {
    300
}
fn default_alerts_send_concurrency_limit() -> usize {
    10
}
fn default_systemctl_retry_attempts() -> u8 {
    30
}
fn default_systemctl_retry_delay() -> u64 {
    5
}
fn default_communications_retry_max() -> u64 {
    60
}
fn default_communications_retry_delay() -> u64 {
    60
}
fn default_sms_recipient_level() -> u8 {
    u8::from(&AlertLevel::Alarm)
}
