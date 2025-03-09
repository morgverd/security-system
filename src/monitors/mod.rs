mod services;
mod power;
mod cctv;
mod sentry;

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use tokio::task::JoinHandle;
use crate::alerts::{AlertInfo, AlertLevel, send_alert};
use crate::config::EnvConfig;
use crate::monitors::sentry::SentryCronMonitor;
use crate::monitors::services::ServicesMonitor;

#[async_trait]
pub(crate) trait Monitor: Send + Sync + 'static {

    /// Returns the static monitor name.
    fn name() -> &'static str;

    /// Creates a new monitor instance with given configuration.
    /// Implementations can override this for custom initialization.
    /// If None is returned, the monitor is not run.
    fn from_config(config: &EnvConfig) -> Option<Self>
    where
        Self: Sized;

    /// Run the monitor forever, returning an Err result to throw to Sentry.
    /// The monitor is always restarted after any return value.
    async fn run(&mut self) -> Result<()>;

    /// Helper method to send alerts with the monitors name as the source.
    async fn send_alert(message: String, level: AlertLevel) -> Result<()> {
        send_alert(AlertInfo {
            source: Self::name().to_string(),
            message,
            level
        }).await
    }
}

async fn run_monitor<T: Monitor>(mut monitor: T) {
    debug!("Spawned monitor: {}", T::name());

    loop {
        match monitor.run().await {
            Ok(_) => info!("Restarting monitor {}!", T::name()),
            Err(e) => error!("Error in monitor {}: {:#?}", T::name(), e)
        }
    }
}

fn spawn_if_enabled<T: Monitor>(config: &EnvConfig) -> Option<JoinHandle<()>> {
    match T::from_config(config) {
        Some(monitor) => Some(tokio::spawn(run_monitor(monitor))),
        None => {
            warn!("Monitor {} is disabled or has invalid configuration!", T::name());
            None
        }
    }
}

pub(crate) async fn spawn_monitors(config: &EnvConfig) -> Vec<JoinHandle<()>> {
    info!("Spawning monitors");

    vec![
        spawn_if_enabled::<SentryCronMonitor>(config),
        spawn_if_enabled::<ServicesMonitor>(config)
    ]
    .into_iter()
    .flatten()
    .collect()
}

