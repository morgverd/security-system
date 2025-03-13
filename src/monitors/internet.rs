use std::time::Duration;
use async_trait::async_trait;
use anyhow::Result;
use log::{debug, info, warn};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use crate::alerts::AlertLevel;
use crate::config::EnvConfig;
use crate::monitors::Monitor;

/*
    Check that the system still has an internet connection to send notifications to.
    Eventually, this should send out notification via SMS if offline.
 */

const PING_ADDR: &str = "google.com:80";

pub(crate) struct InternetMonitor {
    online: bool,
    interval_duration: Duration,
    timeout_duration: Duration
}

#[async_trait]
impl Monitor for InternetMonitor {

    fn name() -> &'static str { "internet" }

    fn from_config(config: &EnvConfig) -> Option<Self>
    where
        Self: Sized
    {
        Some(Self {
            online: true,
            interval_duration: Duration::from_secs(config.internet_poll_interval),
            timeout_duration: Duration::from_secs(config.internet_poll_timeout)
        })
    }

    async fn run(&mut self) -> Result<()> {
        loop {
            let online = match timeout(self.timeout_duration, TcpStream::connect(PING_ADDR)).await {
                Ok(Ok(_)) => true,
                Ok(Err(e)) => {
                    warn!("Ping error: {}", e.to_string());
                    false
                },
                Err(_) => {
                    warn!("Ping timeout!");
                    false
                },
            };

            debug!("Internet status from ping: {}.", if online { "Online" } else { "Offline" });
            if online {
                if !self.online {
                    info!("Now online!");

                    self.online = true;
                    Self::send_alert("Security system has reconnected to the internet.".to_owned(), AlertLevel::Warning).await?;
                }
            } else {
                if self.online {
                    info!("Now offline!");

                    self.online = false;
                    Self::send_alert("The security system has lost it's internet connection.".to_owned(), AlertLevel::Warning).await?;
                }
            }
            sleep(self.interval_duration).await;
        }
    }
}