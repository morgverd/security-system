use std::time::Duration;
use async_trait::async_trait;
use anyhow::Result;
use crate::alerts::AlertInfo;
use crate::monitors::Monitor;

/*
    Check that a set of other important systemd services are still running.
    If they stop running, attempt to restart them. If that fails, send out
    a critical warning only if it's the Alarm Modem, otherwise a warning.
 */

const MONITORED_PROCESSES: [&str; 3] = [
    "security_cctv_smtp",
    "security_cctv_proxy",
    "security_alarm_modem"
];

#[derive(Default)]
pub(crate) struct ProcessesMonitor;

#[async_trait]
impl Monitor for ProcessesMonitor {

    fn name(&self) -> &'static str { "ProcessesMonitor" }
    fn interval(&self) -> Duration { Duration::from_secs(10) }

    async fn run(&mut self) -> Result<Option<AlertInfo>> {
        todo!()
    }
}