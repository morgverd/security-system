use crate::config::MonitorsConfig;
use log::{debug, info, warn};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

/*
   Used in InternetMonitor and CCTVMonitor.
   Attempts to create a TCP connection with ping_addr, returning a PingEvent for Online/Offline.
*/

pub struct PingEvent {
    pub message: String,
}

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
        config: &MonitorsConfig,
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

    /// Polls until a status change occurs, then returns an event with the alert message.
    pub async fn run(&mut self) -> PingEvent {
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

            if online != self.online {
                self.online = online;
                let message = if online {
                    info!("{}: Now online!", self.name);
                    self.reconnected_message
                } else {
                    info!("{}: Now offline!", self.name);
                    self.disconnected_message
                };
                return PingEvent {
                    message: message.to_owned(),
                };
            }

            sleep(self.interval_duration).await;
        }
    }
}
