mod services;
mod power;
mod internet;
mod cctv;

use anyhow::Result;
use std::time::Duration;
use async_trait::async_trait;
use log::{debug, error, info};
use tokio::time::interval;

use crate::alerts::{AlertInfo, AlertLevel, send_alert};
use crate::monitors::internet::InternetMonitor;
use crate::monitors::services::ServicesMonitor;

#[async_trait]
pub(crate) trait Monitor: Send + Sync + 'static {

    fn name(&self) -> &'static str;
    fn interval(&self) -> Duration;

    /// Run the Monitor, returning an Optional alert to send.
    async fn run(&mut self) -> Result<Option<AlertInfo>>;

    /// Create an alert to return in run with the monitor name as the source.
    fn create_alert(&self, message: String, level: AlertLevel) -> AlertInfo {
        AlertInfo {
            source: self.name(),
            message,
            level
        }
    }
}

async fn run_monitor(mut monitor: impl Monitor) {
    let mut interval = interval(monitor.interval());
    debug!("Spawned monitor: {}", monitor.name());

    loop {
        interval.tick().await;
        match monitor.run().await {
            Ok(Some(alert)) => {
                info!("Sending alert from monitor: {}", monitor.name());
                send_alert(alert).await;
            },
            Ok(None) => (),
            Err(e) => error!("Error running monitor {}: {e:?}", monitor.name())
        }
    }
}

pub(crate) async fn spawn_monitors() {
    info!("Spawning monitors");

    tokio::spawn(run_monitor(InternetMonitor::default()));
    tokio::spawn(run_monitor(ServicesMonitor::default()));
}

