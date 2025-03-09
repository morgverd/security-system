use std::env;
use std::fmt::Display;
use std::net::SocketAddr;
use std::str::FromStr;
use anyhow::Result;
use log::LevelFilter;

#[derive(Clone)]
pub struct EnvConfig {
    pub server_addr: SocketAddr,
    pub log_level: LevelFilter,
    pub sentry_dsn: Option<String>,
    pub sentry_cron_url: Option<String>,
    pub sentry_cron_interval: u64,

    /// How often should systemd services be checked? (Used in ServicesMonitor).
    pub services_poll_interval: u64
}

fn get_env<T: FromStr>(key: &str) -> Option<T>
where
    T::Err: Display
{
    env::var(key).ok().and_then(|v| v.parse::<T>().ok())
}

fn get_env_default<T: FromStr>(key: &str, default: T) -> Result<T>
where
    T::Err: Display
{
    Ok(get_env(key).unwrap_or(default))
}

pub fn from_env() -> Result<EnvConfig> {
    Ok(EnvConfig {
        server_addr: get_env_default("SECURITY_HTTP_ADDR", "127.0.0.1:9050".parse().expect("Invalid default address!"))?,
        log_level: get_env_default("SECURITY_LOG_LEVEL", LevelFilter::Info)?,
        sentry_dsn: get_env("SECURITY_SENTRY_DSN"),
        sentry_cron_url: get_env("SECURITY_SENTRY_CRON_URL"),
        sentry_cron_interval: get_env_default("SECURITY_SENTRY_CRON_INTERVAL", 180)?,
        services_poll_interval: get_env_default("SECURITY_SERVICES_POLL_INTERVAL", 60)?,
    })
}