use std::time::Duration;
use async_trait::async_trait;
use anyhow::Result;
use log::{debug, error, info, warn};
use tokio::process::Command;
use tokio::time::{interval, sleep};
use crate::alerts::AlertLevel;
use crate::config::EnvConfig;
use crate::monitors::Monitor;

/*
    Check that a set of other important systemd services are still running.
    If they stop running, attempt to restart them. If that fails, send out
    a critical warning only if it's the Alarm Modem, otherwise a warning.
 */

const RETRY_ATTEMPTS: u8 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(5);

const MONITORED_SERVICES: [(&str, AlertLevel); 1] = [
    // "security_cctv_smtp",
    // ("security_cctv_proxy", AlertLevel::Warning),
    ("security_alarm_modem", AlertLevel::Critical)
];

struct MonitoredServiceState {
    name: String,
    level: AlertLevel,
    is_offline: bool,
    retry_count: u8
}

pub(crate) struct ServicesMonitor {
    services: Vec<MonitoredServiceState>,
    interval: u64
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

    async fn attempt_service_restart(name: &str) -> Result<bool> {
        let output = Command::new("systemctl")
            .arg("restart")
            .arg(name)
            .output()
            .await?;

        Ok(output.status.success())
    }

    async fn handle_offline_service(service: &mut MonitoredServiceState) -> Result<()> {
        service.retry_count += 1;

        // Keep retrying restarts until the retry limit is met.
        info!(
            "Service {} is offline, restart attempt {}/{}.",
            service.name,
            service.retry_count,
            RETRY_ATTEMPTS
        );
        if service.retry_count <= RETRY_ATTEMPTS {
            sleep(RETRY_DELAY).await;

            info!("Attempting to restart service {}!", &service.name);
            if Self::attempt_service_restart(&service.name).await? {

                info!("Service {} was successfully restarted!", &service.name);
                service.retry_count = 0;
                return Ok(());
            }
        }

        // The service is now offline, either due to exceeding RETRY_ATTEMPTS
        // or because the systemctl restart service command directly failed.
        if !service.is_offline {
            service.is_offline = true;
            Self::send_alert(format!("{} is OFFLINE after {} attempts to restart!", service.name, service.retry_count), service.level.clone()).await?;
        }

        Ok(())
    }

    async fn check_service(service: &mut MonitoredServiceState) -> Result<()> {

        // Do the actual service status checking here.
        debug!("Checking service {} state...", &service.name);
        match Self::is_service_active(&service.name).await {
            Ok(true) => {

                // The service is now online.
                debug!("Service {} is online!", &service.name);
                if service.is_offline {
                    service.is_offline = false;
                    service.retry_count = 0;

                    Self::send_alert(format!("{} is now ONLINE!", service.name), service.level.clone()).await?;
                }
            },
            Ok(false) => Self::handle_offline_service(service).await?,
            Err(e) => error!("Failed to check service status {}: {}", service.name, e)
        }
        Ok(())
    }
}

#[async_trait]
impl Monitor for ServicesMonitor {

    #[inline]
    fn name() -> &'static str { "ServicesMonitor" }

    fn from_config(config: &EnvConfig) -> Option<Self> {
        let services: Vec<MonitoredServiceState> = MONITORED_SERVICES
            .into_iter()
            .map(|(service_name, service_level)| {
                MonitoredServiceState {
                    name: service_name.to_string(),
                    level: service_level,
                    is_offline: false,
                    retry_count: 0
                }
            })
            .collect();

        // Only enable if there are services to monitor.
        if services.is_empty() {
            warn!("There are no services defined to monitor!");
            None
        } else {
            Some(Self { services, interval: config.services_poll_interval })
        }
    }

    async fn run(&mut self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(self.interval));

        debug!("Started with an interval of {} seconds!", self.interval);
        loop {
            for service in &mut self.services {
                Self::check_service(service).await?;
            }
            interval.tick().await;
        }
    }
}