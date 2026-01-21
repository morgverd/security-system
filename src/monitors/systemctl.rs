use crate::alerts::AlertLevel;
use crate::config::MonitorsConfig;
use crate::monitors::Monitor;
use log::{debug, error, info};

/*
   Check that a set of other important systemctl services are still running.
   If they stop running, attempt to restart them. If that fails, send out
   alerts with differing AlertLevels based on the importance of the service.
*/

struct MonitoredSystemctlState {
    name: String,
    level: AlertLevel,
    is_offline: bool,
    retry_count: u8,
}

pub(crate) struct SystemctlMonitor {
    services: Vec<MonitoredSystemctlState>,
    interval: u64,
    retry_attempts: u8,
    retry_delay: std::time::Duration,
}
impl SystemctlMonitor {
    async fn is_service_active(name: &str) -> anyhow::Result<bool> {
        let output = tokio::process::Command::new("systemctl")
            .arg("is-active")
            .arg(name)
            .output()
            .await?;

        Ok(output.status.success())
    }

    async fn attempt_service_restart(name: &str) -> anyhow::Result<bool> {
        let output = tokio::process::Command::new("systemctl")
            .arg("restart")
            .arg(name)
            .output()
            .await?;

        Ok(output.status.success())
    }

    async fn handle_offline_service(&mut self, index: usize) -> anyhow::Result<()> {
        self.services[index].retry_count += 1;

        let service = &self.services[index];
        let service_name = service.name.clone();

        info!(
            "Service {} is offline, restart attempt {}/{}.",
            service_name, service.retry_count, self.retry_attempts
        );

        if service.retry_count <= self.retry_attempts {
            tokio::time::sleep(self.retry_delay).await;

            info!("Attempting to restart service {}!", &service_name);
            if Self::attempt_service_restart(&service_name).await? {
                info!("Service {} was successfully restarted!", &service_name);
                self.services[index].retry_count = 0;
                return Ok(());
            }
        }

        let service = &mut self.services[index];
        if !service.is_offline {
            service.is_offline = true;
            Self::send_alert(
                format!(
                    "{} is OFFLINE after {} attempts to restart!",
                    service_name, service.retry_count
                ),
                service.level.clone(),
            )
            .await?;
        }

        Ok(())
    }

    async fn check_service(&mut self, index: usize) -> anyhow::Result<()> {
        // Do the actual service status checking here.
        let service_name = self.services[index].name.clone();
        debug!("Checking service {} state...", &service_name);

        match Self::is_service_active(&service_name).await {
            Ok(true) => {
                // The service is now online.
                debug!("Service {} is online!", &service_name);
                let service = &mut self.services[index];
                if service.is_offline {
                    service.is_offline = false;
                    service.retry_count = 0;

                    Self::send_alert(
                        format!("{service_name} is now ONLINE!"),
                        service.level.clone(),
                    )
                    .await?;
                }
            }
            Ok(false) => self.handle_offline_service(index).await?,
            Err(e) => error!("Failed to check service status {service_name}: {e}"),
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Monitor for SystemctlMonitor {
    #[inline]
    fn name() -> &'static str {
        "system_ctl"
    }

    fn from_config(config: &MonitorsConfig) -> anyhow::Result<Self> {
        let services = config
            .systemctl
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing services_monitored!"))?
            .iter()
            .map(|service| {
                Ok(MonitoredSystemctlState {
                    name: service.name.to_string(),
                    level: AlertLevel::try_from(service.level)?,
                    is_offline: false,
                    retry_count: 0,
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;

        // Only enable if there are services to monitor.
        if services.is_empty() {
            anyhow::bail!("There are no services defined to monitor!");
        }

        Ok(Self {
            services,
            interval: config.systemctl_poll_interval,
            retry_attempts: config.systemctl_retry_attempts,
            retry_delay: std::time::Duration::from_secs(config.systemctl_retry_delay),
        })
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(self.interval));

        debug!("Started with an interval of {} seconds!", self.interval);
        loop {
            for i in 0..self.services.len() {
                self.check_service(i).await?;
            }
            interval.tick().await;
        }
    }
}
