use crate::alerts::AlertLevel;
use crate::config::AppConfig;
use anyhow::Result;
use log::{debug, info, warn};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

/*
   Used in InternetMonitor and CCTVMonitor.
   Attempts to create a TCP connection with ping_addr, sending alerts for disconnect/reconnect.
*/

pub struct PingMonitor {
    name: &'static str,
    ping_addr: String,
    reconnected_message: &'static str,
    disconnected_message: &'static str,
    online: bool,
    interval_duration: Duration,
    timeout_duration: Duration,
}
impl PingMonitor {
    pub fn new(
        name: &'static str,
        ping_addr: String,
        reconnected_message: &'static str,
        disconnected_message: &'static str,
        config: &AppConfig,
    ) -> Self {
        Self {
            name,
            ping_addr,
            reconnected_message,
            disconnected_message,
            online: true,
            interval_duration: Duration::from_secs(config.ping_poll_interval),
            timeout_duration: Duration::from_secs(config.ping_poll_timeout),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let online =
                match timeout(self.timeout_duration, TcpStream::connect(&self.ping_addr)).await {
                    Ok(Ok(_)) => true,
                    Ok(Err(e)) => {
                        warn!("Ping error to {}: {}", self.ping_addr, e);
                        false
                    }
                    Err(_) => {
                        warn!("Ping timeout to {}!", self.ping_addr);
                        false
                    }
                };

            debug!(
                "{} status from ping to {}: {}.",
                self.name,
                self.ping_addr,
                if online { "Online" } else { "Offline" }
            );

            if online {
                if !self.online {
                    info!("{}: Now online!", self.name);
                    self.online = true;
                    self.send_alert(self.reconnected_message.to_owned(), AlertLevel::Info)
                        .await?;
                }
            } else if self.online {
                info!("{}: Now offline!", self.name);
                self.online = false;
                self.send_alert(self.disconnected_message.to_owned(), AlertLevel::Info)
                    .await?;
            }

            sleep(self.interval_duration).await;
        }
    }

    async fn send_alert(&self, _message: String, _level: AlertLevel) -> Result<()> {
        unimplemented!("implement alert sending")
    }
}
