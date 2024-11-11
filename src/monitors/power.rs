use std::time::Duration;
use async_trait::async_trait;
use anyhow::Result;
use log::info;
use crate::alerts::AlertInfo;
use crate::monitors::Monitor;

/*
    Check that the Raspberry Pi still has a direct power connection and
    isn't running from battery. Ideally, send a warning notification
    and emergency when it gets close to running out of power.
 */

#[derive(Default)]
pub(crate) struct PowerMonitor {
    test: usize
}

#[async_trait]
impl Monitor for PowerMonitor {

    fn name(&self) -> &'static str { "PowerMonitor" }
    fn interval(&self) -> Duration { Duration::from_secs(4) }

    async fn run(&mut self) -> Result<Option<AlertInfo>> {
        info!("Hello from PowerMonitor {}", self.test);
        self.test += 1;

        Ok(None)
    }
}