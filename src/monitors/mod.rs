mod cctv;
mod cron;
mod internet;
mod ping;
mod power;
mod services;

use crate::alerts::{send_alert, AlertInfo, AlertLevel};
use crate::config::MonitorsConfig;
use crate::monitors::cctv::CCTVMonitor;
use crate::monitors::cron::CronMonitor;
use crate::monitors::internet::InternetMonitor;
use crate::monitors::services::ServicesMonitor;
use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use std::collections::HashSet;
use tokio::task::JoinHandle;

#[async_trait]
pub(crate) trait Monitor: Send + Sync + 'static {
    /// Returns the static monitor name for logging.
    fn name() -> &'static str;

    /// Creates a new monitor instance with given configuration.
    /// Implementations can override this for custom initialization.
    /// If None is returned, the monitor is not run.
    fn from_config(config: &MonitorsConfig) -> Option<Self>
    where
        Self: Sized;

    /// Run the monitor forever, returning an Err result to throw to Sentry.
    /// The monitor is always restarted after any return value.
    async fn run(&mut self) -> Result<()>;

    /// Helper method to send alerts with the monitors name as the source.
    async fn send_alert(message: String, level: AlertLevel) -> Result<()> {
        let name = Self::name().to_string();
        let alert = AlertInfo::new(format!("{name}-monitor"), message, level)?;
        send_alert(alert).await
    }
}

async fn run_monitor<T: Monitor>(mut monitor: T) {
    debug!("Running {} monitor!", T::name());
    loop {
        match monitor.run().await {
            Ok(_) => info!("Restarting {} monitor!", T::name()),
            Err(e) => error!("Error in {} monitor: {:#?}", T::name(), e),
        }
    }
}

fn try_from_config<T: Monitor>(
    config: &MonitorsConfig,
    disabled_monitors: Option<&HashSet<String>>,
) -> Option<JoinHandle<()>> {
    let name = T::name();
    if let Some(disabled_monitors) = disabled_monitors {
        if disabled_monitors.contains(name) {
            warn!("The {name} monitor is disabled by config!");
            return None;
        }
    }

    match T::from_config(config) {
        Some(monitor) => Some(tokio::spawn(run_monitor(monitor))),
        None => {
            warn!("The {name} monitor is disabled or has invalid configuration!");
            None
        }
    }
}

pub(crate) async fn spawn_monitors(config: &MonitorsConfig) -> Vec<JoinHandle<()>> {
    let disabled_monitors = config.disabled.as_ref();
    vec![
        try_from_config::<CCTVMonitor>(config, disabled_monitors),
        try_from_config::<InternetMonitor>(config, disabled_monitors),
        try_from_config::<CronMonitor>(config, disabled_monitors),
        try_from_config::<ServicesMonitor>(config, disabled_monitors),
    ]
    .into_iter()
    .flatten()
    .collect()
}
