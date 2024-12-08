use std::time::Duration;
use async_trait::async_trait;
use anyhow::Result;
use log::{debug, info};
use tokio::process::Command;
use crate::alerts::{AlertInfo, AlertLevel};
use crate::monitors::Monitor;

/*
    Check that a set of other important systemd services are still running.
    If they stop running, attempt to restart them. If that fails, send out
    a critical warning only if it's the Alarm Modem, otherwise a warning.
 */

const MONITORED_SERVICES: [&str; 1] = [
    // "security_cctv_smtp",
    // "security_cctv_proxy",
    "security_alarm_modem"
];

#[derive(Default)]
pub(crate) struct ServicesMonitor {
    offline_services: Vec<String>
}
impl ServicesMonitor {
    async fn is_service_active(name: &str) -> Result<bool> {
        let output = Command::new("systemctl")
            .arg("is-active")
            .arg(name)
            .output()
            .await?;

        Ok(output.status.success())
    }

    fn create_services_alert(&self, now_offline: &[String], now_online: &[String]) -> Option<AlertInfo> {
        if now_offline.is_empty() && now_online.is_empty() {
            debug!("No service status changes!");
            return None;
        }

        let message = now_offline.iter()
            .chain(now_online.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");

        info!("Services: {message}");
        Some(self.create_alert(message, AlertLevel::Critical))
    }
}

#[async_trait]
impl Monitor for ServicesMonitor {

    fn name(&self) -> &'static str { "ServicesMonitor" }
    fn interval(&self) -> Duration { Duration::from_secs(10) }

    async fn run(&mut self) -> Result<Option<AlertInfo>> {

        let mut currently_offline = Vec::<String>::with_capacity(MONITORED_SERVICES.len());
        for service in MONITORED_SERVICES {
            if !Self::is_service_active(service).await? {
                currently_offline.push(service.to_string());
            }
        }

        let now_offline: Vec<String> = currently_offline
            .iter()
            .filter(|service| !self.offline_services.contains(*service))
            .map(|service| format!("{service} now OFFLINE"))
            .collect();

        let now_online: Vec<String> = self.offline_services
            .iter()
            .filter(|service| !currently_offline.contains(service))
            .map(|service| format!("{service} ONLINE"))
            .collect();

        self.offline_services = currently_offline;
        Ok(self.create_services_alert(&now_offline, &now_online))
    }
}