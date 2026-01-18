use crate::config::AppConfig;
use crate::monitors::ping::PingMonitor;
use crate::monitors::Monitor;
use anyhow::Result;
use async_trait::async_trait;

/*
   Check that the building still has an internet connection.
   Alerts are sent by SMS now, so this is just for general building monitoring.
*/

pub(crate) struct InternetMonitor(PingMonitor);

#[async_trait]
impl Monitor for InternetMonitor {
    fn name() -> &'static str {
        "internet"
    }

    fn from_config(config: &AppConfig) -> Option<Self> {
        Some(Self(PingMonitor::new(
            "internet",
            "google.com:80".to_string(),
            "The security system has reconnected to the internet.",
            "The security system has lost its internet connection.",
            config,
        )))
    }

    async fn run(&mut self) -> Result<()> {
        self.0.run().await
    }
}
