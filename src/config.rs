//! TODO: Make into a config.toml file.
//! TODO: Make get_sms_recipients read from config file

use crate::alerts::AlertRecipient;
use anyhow::Result;
use log::LevelFilter;
use sms_client::config::{ClientConfig, TLSConfig};
use std::collections::HashSet;
use std::env;
use std::fmt::Display;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

fn get_env<T: FromStr>(key: &str) -> Option<T>
where
    T::Err: Display,
{
    env::var(key).ok().and_then(|v| v.parse::<T>().ok())
}

fn get_env_default<T: FromStr>(key: &str, default: T) -> Result<T>
where
    T::Err: Display,
{
    Ok(get_env(key).unwrap_or(default))
}

#[derive(Clone)]
pub(crate) struct AppConfig {
    pub http_addr: SocketAddr,
    pub log_level: LevelFilter,
    pub sentry_dsn: Option<String>,
    pub sentry_cron_url: Option<String>,
    pub sentry_cron_interval: u64,

    // Alerts ----------------------------------
    pub alarm_cooldown: u64,
    pub alerts_retry_max: u64,
    pub alerts_retry_base_delay: u64,
    pub alerts_retry_max_delay: u64,
    pub alerts_concurrency_limit: usize,

    // Monitors --------------------------------
    pub disabled_monitors: Option<HashSet<String>>,
    pub services_poll_interval: u64,
    pub internet_poll_interval: u64,
    pub internet_poll_timeout: u64,

    // SMS Communication --------------------------
    pub sms_http_base: String,
    pub sms_auth: Option<String>,
    pub sms_certificate_path: Option<PathBuf>,
}
impl AppConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            http_addr: get_env_default(
                "SECURITY_HTTP_ADDR",
                "127.0.0.1:9050".parse().expect("Invalid default address!"),
            )?,
            log_level: get_env_default("SECURITY_LOG_LEVEL", LevelFilter::Info)?,

            sentry_dsn: get_env("SECURITY_SENTRY_DSN"),
            sentry_cron_url: get_env("SECURITY_SENTRY_CRON_URL"),
            sentry_cron_interval: get_env_default("SECURITY_SENTRY_CRON_INTERVAL", 180)?,

            alarm_cooldown: get_env_default("SECURITY_ALARM_COOLDOWN", 300)?,
            alerts_retry_max: get_env_default("SECURITY_ALERTS_RETRY_MAX", 8)?,
            alerts_retry_base_delay: get_env_default("SECURITY_ALERTS_RETRY_BASE_DELAY", 2)?,
            alerts_retry_max_delay: get_env_default("SECURITY_ALERTS_RETRY_MAX_DELAY", 90)?,
            alerts_concurrency_limit: get_env_default("SECURITY_ALERTS_CONCURRENCY_LIMIT", 3)?,

            disabled_monitors: get_env("SECURITY_DISABLED_MONITORS").map(|s: String| {
                s.split(",")
                    .map(|v| v.trim().to_string())
                    .collect::<HashSet<_>>()
            }),
            services_poll_interval: get_env_default("SECURITY_SERVICES_POLL_INTERVAL", 60)?,
            internet_poll_interval: get_env_default("SECURITY_INTERNET_POLL_INTERVAL", 180)?,
            internet_poll_timeout: get_env_default("SECURITY_INTERNET_POLL_TIMEOUT", 10)?,

            sms_http_base: get_env("SECURITY_SMS_HTTP_BASE")
                .expect("Missing required SECURITY_SMS_HTTP_BASE env var!"),
            sms_auth: get_env("SECURITY_SMS_AUTH"),
            sms_certificate_path: get_env("SECURITY_SMS_CERTIFICATE_PATH"),
        })
    }

    pub fn get_sms_config(&self) -> ClientConfig {
        let mut config = ClientConfig::http_only(&self.sms_http_base);
        if let Some(auth) = &self.sms_auth {
            config = config.with_auth(auth);
        }
        if let Some(certificate_path) = &self.sms_certificate_path {
            config = config.add_tls(
                TLSConfig::new(certificate_path).expect("Invalid SMS certificate filepath!"),
            );
        }
        config
    }

    pub fn get_sms_recipients(&self) -> Vec<AlertRecipient> {
        vec![(1, "+test".to_owned())]
    }
}
