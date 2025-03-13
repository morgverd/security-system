use std::{env, fs};
use std::collections::HashSet;
use std::fmt::Display;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use anyhow::Result;
use log::LevelFilter;

#[derive(Clone)]
pub(crate) struct EnvConfig {
    pub server_addr: SocketAddr,
    pub log_level: LevelFilter,
    pub sentry_dsn: Option<String>,
    pub sentry_cron_url: Option<String>,
    pub sentry_cron_interval: u64,

    // Alerts ----------------------------------

    pub alarm_cooldown: u64,
    pub alerts_states_dir: PathBuf,
    pub alerts_retry_delay: u64,
    pub alerts_retry_max: u64,
    pub alerts_concurrency_limit: usize,

    // Monitors --------------------------------

    pub disabled_monitors: Option<HashSet<String>>,
    pub services_poll_interval: u64,
    pub internet_poll_interval: u64,
    pub internet_poll_timeout: u64,

    // Communications --------------------------

    // Pushover
    pub pushover_token: Option<String>,
    pub pushover_users: Option<Vec<String>>,

    // Text Anywhere
    pub text_anywhere_originator: Option<String>,
    pub text_anywhere_username: Option<String>,
    pub text_anywhere_password: Option<String>,
    pub text_anywhere_destinations: Option<String>
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

        alerts_states_dir: get_env("SECURITY_ALARM_STATES_DIR")
            .unwrap_or_else(|| {

                // If there is no states directory, create one within system temp.
                let mut temp_dir = env::temp_dir();
                temp_dir.push("security_alarm_states");
                fs::create_dir_all(&temp_dir).expect("Failed to create temporary alarm states directory!");
                temp_dir
            }),
        alarm_cooldown: get_env_default("SECURITY_ALARM_COOLDOWN", 300)?,
        alerts_retry_max: get_env_default("SECURITY_ALERTS_RETRY_MAX", 720)?,
        alerts_retry_delay: get_env_default("SECURITY_ALERTS_RETRY_DELAY", 30)?,
        alerts_concurrency_limit: get_env_default("SECURITY_ALERTS_CONCURRENCY_LIMIT", 3)?,

        disabled_monitors: get_env("SECURITY_DISABLED_MONITORS")
            .map(|s: String| {
                s.split(",").into_iter().map(|v| v.trim().to_string()).collect::<HashSet<_>>()
            }),
        services_poll_interval: get_env_default("SECURITY_SERVICES_POLL_INTERVAL", 60)?,
        internet_poll_interval: get_env_default("SECURITY_INTERNET_POLL_INTERVAL", 180)?,
        internet_poll_timeout: get_env_default("SECURITY_INTERNET_POLL_TIMEOUT", 10)?,

        pushover_token: get_env("SECURITY_PUSHOVER_TOKEN"),
        pushover_users: get_env::<String>("SECURITY_PUSHOVER_USERS")
            .map(|s| {
                s.split(",").into_iter().map(|v| v.trim().to_string()).collect()
            }),

        text_anywhere_originator: get_env("SECURITY_TEXT_ANYWHERE_ORIGINATOR"),
        text_anywhere_username: get_env("SECURITY_TEXT_ANYWHERE_USERNAME"),
        text_anywhere_password: get_env("SECURITY_TEXT_ANYWHERE_PASSWORD"),
        text_anywhere_destinations: get_env("SECURITY_TEXT_ANYWHERE_DESTINATIONS")
    })
}