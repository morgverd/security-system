use crate::alerts::AlertLevel;
use crate::config::{MonitoredPingTarget, MonitorsConfig};
use crate::monitors::Monitor;
use log::{debug, warn};

/*
   Attempt TCP connections to an addr per interval with a timeout.
*/

#[derive(Clone)]
struct PingTarget {
    name: String,
    addr: String,
    level: AlertLevel,
    timeout: std::time::Duration,
    interval: std::time::Duration,
}
impl TryFrom<&MonitoredPingTarget> for PingTarget {
    type Error = anyhow::Error;

    fn try_from(value: &MonitoredPingTarget) -> Result<Self, Self::Error> {
        AlertLevel::try_from(value.level).map(|level| PingTarget {
            name: value.name.clone(),
            addr: value.addr.clone(),
            level,
            timeout: std::time::Duration::from_secs(value.timeout.unwrap_or(5)),
            interval: std::time::Duration::from_secs(value.interval.unwrap_or(60)),
        })
    }
}

pub(crate) struct PingMonitor {
    targets: Vec<PingTarget>,
}
impl PingMonitor {
    async fn run_target(target: PingTarget) -> anyhow::Result<()> {
        let mut is_online = true;
        let seconds = target.interval.as_secs();
        loop {
            let currently_online = match tokio::time::timeout(
                target.timeout,
                tokio::net::TcpStream::connect(&target.addr),
            )
            .await
            {
                Ok(Ok(_)) => true,
                Ok(Err(e)) => {
                    warn!("[{}] Ping error to {}: {}", target.name, target.addr, e);
                    false
                }
                Err(_) => {
                    warn!(
                        "[{}] Ping timeout ({:?}) to {}!",
                        target.name, target.timeout, target.addr
                    );
                    false
                }
            };

            debug!(
                "[{}, {seconds}s] Ping to {}: {}",
                target.name,
                target.addr,
                if currently_online {
                    "Online"
                } else {
                    "Offline"
                }
            );

            if currently_online != is_online {
                is_online = currently_online;
                let message = if currently_online {
                    format!("[{}] Now online!", target.name)
                } else {
                    format!("[{}] Now offline!", target.name)
                };

                debug!("{message}");
                Self::send_alert(message, target.level.clone()).await?;
            }

            tokio::time::sleep(target.interval).await;
        }
    }
}

#[async_trait::async_trait]
impl Monitor for PingMonitor {
    fn name() -> &'static str {
        "ping"
    }

    fn from_config(config: &MonitorsConfig) -> anyhow::Result<Self> {
        let targets: Vec<_> = config
            .pings
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing pings_monitored!"))?
            .iter()
            .map(PingTarget::try_from)
            .collect::<Result<_, _>>()?;

        if targets.is_empty() {
            anyhow::bail!("No ping targets configured!");
        }

        Ok(Self { targets })
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        let handles: Vec<_> = self
            .targets
            .iter()
            .cloned()
            .map(|target| tokio::spawn(async move { Self::run_target(target).await }))
            .collect();

        // Wait for any task to complete (they shouldn't unless there's an error)
        for result in futures::future::join_all(handles).await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e),
                Err(e) => anyhow::bail!("Task panicked: {}", e),
            }
        }

        Ok(())
    }
}
