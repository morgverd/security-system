use std::time::Duration;
use async_trait::async_trait;
use anyhow::Result;
use log::{info, warn};
use tokio::net::TcpStream;
use tokio::time::timeout;
use crate::alerts::{AlertInfo, AlertLevel};
use crate::monitors::Monitor;

/*
    Check that the system still has an internet connection to send notifications to.
    Eventually, this should send out notification via SMS if offline.
 */

const PING_ADDR: &str = "google.com:80";

pub(crate) struct InternetMonitor {
    online: bool
}

#[async_trait]
impl Monitor for InternetMonitor {

    fn name(&self) -> &'static str { "InternetMonitor" }
    fn interval(&self) -> Duration { Duration::from_secs(10) }

    async fn run(&mut self) -> Result<Option<AlertInfo>> {

        // Send test ping.
        let online = match timeout(Duration::from_secs(5), TcpStream::connect(PING_ADDR)).await {
            Ok(Ok(_)) => true,
            Ok(Err(e)) => { warn!("Ping error: {}", e.to_string()); false },
            Err(_) => { warn!("Ping timeout!"); false },
        };

        if online {
            if !self.online {
                info!("Now online!");

                self.online = true;
                return Ok(Some( // Now online
                    self.create_alert("Security system has reconnected to the internet.".to_owned(), AlertLevel::Warning)
                ));
            }
        } else {
            if self.online {
                info!("Now offline!");

                self.online = false;
                return Ok(Some( // Now offline
                    self.create_alert("The security system has lost it's internet connection.".to_owned(), AlertLevel::Warning)
                ));
            }
        }
        Ok(None)
    }
}

impl Default for InternetMonitor {
    fn default() -> Self {
        Self { online: true }
    }
}