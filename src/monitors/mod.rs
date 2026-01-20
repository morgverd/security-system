mod cron;
mod ping;
mod power;
mod services;

use crate::alerts::{send_alert, AlertInfo, AlertLevel};
use crate::config::MonitorsConfig;
use log::{debug, error, info, warn};

#[async_trait::async_trait]
pub(crate) trait Monitor: Send + Sync + 'static {
    /// Returns the static monitor name for logging.
    fn name() -> &'static str;

    /// Creates a new monitor instance with given configuration.
    /// Implementations can override this for custom initialization.
    /// If None is returned, the monitor is not run.
    fn from_config(config: &MonitorsConfig) -> anyhow::Result<Self>
    where
        Self: Sized;

    /// Run the monitor forever, returning an Err result to throw to Sentry.
    /// The monitor is always restarted after any return value.
    async fn run(&mut self) -> anyhow::Result<()>;

    /// Helper method to send alerts with the monitors name as the source.
    async fn send_alert(message: String, level: AlertLevel) -> anyhow::Result<()> {
        let name = Self::name().to_string();
        let alert = AlertInfo::new(format!("{name}-monitor"), message, level)?;
        send_alert(alert).await
    }
}

async fn run_monitor<T: Monitor>(mut monitor: T) {
    let name = T::name();
    debug!("Running {name} monitor!");
    loop {
        match monitor.run().await {
            Ok(_) => info!("Restarting '{name}' monitor!"),
            Err(e) => error!("Error in '{name}' monitor: {:#?}", e),
        }
    }
}

fn try_from_config<T: Monitor>(
    config: &MonitorsConfig,
    disabled_monitors: Option<&std::collections::HashSet<String>>,
) -> Option<tokio::task::JoinHandle<()>> {
    let name = T::name();
    if let Some(disabled_monitors) = disabled_monitors {
        if disabled_monitors.contains(name) {
            warn!("Monitor '{name}' is disabled by config!");
            return None;
        }
    }

    match T::from_config(config) {
        Ok(monitor) => Some(tokio::spawn(run_monitor(monitor))),
        Err(e) => {
            warn!("Monitor '{name}' failed to initialize: {e:?}");
            None
        }
    }
}

pub(crate) async fn spawn_monitors(config: &MonitorsConfig) -> Vec<tokio::task::JoinHandle<()>> {
    let disabled_monitors = config.disabled.as_ref();
    vec![
        try_from_config::<ping::PingMonitor>(config, disabled_monitors),
        try_from_config::<cron::CronMonitor>(config, disabled_monitors),
        try_from_config::<services::ServicesMonitor>(config, disabled_monitors),
    ]
    .into_iter()
    .flatten()
    .collect()
}
