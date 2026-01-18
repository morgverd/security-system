use crate::alerts::AlertLevel;
use crate::config::MonitorsConfig;
use crate::monitors::ping::PingMonitor;
use crate::monitors::Monitor;
use async_trait::async_trait;

/*
   Check the CCTV DVR status. We should receive camera blocking / signal loss
   alerts via SMTP so this is just to make sure that it's still online & connected
   to the network.
*/

pub(crate) struct CCTVMonitor(PingMonitor);

#[async_trait]
impl Monitor for CCTVMonitor {
    fn name() -> &'static str {
        "cctv"
    }

    fn from_config(config: &MonitorsConfig) -> Option<Self> {
        Some(Self(PingMonitor::new(
            "cctv",
            config.cctv_local_ip.clone()?,
            "The CCTV NVR is now online.",
            "The CCTV NVR is not responding to pings.",
            config,
        )))
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let event = self.0.run().await;
            Self::send_alert(event.message, AlertLevel::Critical).await?;
        }
    }
}
