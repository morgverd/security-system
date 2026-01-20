use anyhow::anyhow;
use crate::alerts::AlertLevel;
use crate::config::{MonitoredPingTarget, MonitorsConfig};
use crate::monitors::Monitor;
use log::{debug, warn};

/*
   Ping monitor that tracks multiple addresses across
   the network sending alerts at configured level.
*/

// TODO: Add timeout and interval to this target directly instead of globally.
struct PingTarget {
    name: String,
    addr: String,
    online: bool,
    level: AlertLevel
}
impl TryFrom<&MonitoredPingTarget> for PingTarget {
    type Error = anyhow::Error;

    fn try_from(value: &MonitoredPingTarget) -> Result<Self, Self::Error> {
        AlertLevel::try_from(value.level)
            .map(|level| PingTarget {
                name: value.name.clone(),
                addr: value.addr.clone(),
                online: true,
                level
            })
    }
}

pub(crate) struct PingMonitor {
    targets: Vec<PingTarget>,
    interval_duration: std::time::Duration,
    timeout_duration: std::time::Duration,
}

#[async_trait::async_trait]
impl Monitor for PingMonitor {
    fn name() -> &'static str {
        "ping"
    }

    fn from_config(config: &MonitorsConfig) -> anyhow::Result<Self> {
        // I'm getting nasty at method chaining
        let targets: Vec<_> = config
            .pings_monitored
            .as_ref()
            .ok_or_else(|| anyhow!("Missing pings_monitored!"))?
            .iter()
            .map(|a| PingTarget::try_from(a))
            .collect::<Result<_, _>>()?;

        if targets.is_empty() {
            anyhow::bail!("No ping targets configured!");
        }

        Ok(Self {
            targets,
            interval_duration: std::time::Duration::from_secs(config.pings_poll_interval),
            timeout_duration: std::time::Duration::from_secs(config.pings_poll_timeout),
        })
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            for target in &mut self.targets {
                let online = match tokio::time::timeout(
                    self.timeout_duration,
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
                        warn!("[{}] Ping timeout to {}!", target.name, target.addr);
                        false
                    }
                };

                debug!(
                    "[{}] Ping to {}: {}",
                    target.name,
                    target.addr,
                    if online { "Online" } else { "Offline" }
                );

                if online != target.online {
                    target.online = online;
                    let message = if online {
                        format!("[{}] Now online!", target.name)
                    } else {
                        format!("[{}] Now offline!", target.name)
                    };

                    debug!("{message}");
                    Self::send_alert(message.clone(), target.level.clone()).await?;
                }
            }

            tokio::time::sleep(self.interval_duration).await;
        }
    }
}