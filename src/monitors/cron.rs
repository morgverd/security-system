use crate::config::MonitorsConfig;
use crate::monitors::Monitor;
use log::{debug, warn};

/*
   Send Sentry CRON requests per interval.
   This is used as remote health-checks for the system.
*/

pub(crate) struct CronMonitor {
    client: reqwest::Client,
    url: String,
    interval: u64,
}

#[async_trait::async_trait]
impl Monitor for CronMonitor {
    fn name() -> &'static str {
        "cron"
    }

    fn from_config(config: &MonitorsConfig) -> Option<Self> {
        let cron_url = config.cron_url.as_ref()?;

        // TODO: Add timeout to client via builder. See PushoverCommunicationProvider.
        Some(CronMonitor {
            client: reqwest::Client::new(),
            url: cron_url.clone(),
            interval: config.cron_interval,
        })
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        let error_interval = std::cmp::max(self.interval / 2, 1);

        debug!("Started with an interval of {} seconds!", self.interval);
        loop {
            let mut current_interval = self.interval;
            match self.client.get(&self.url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!("Successfully sent update!");
                    } else {
                        warn!("Failed to send CRON request with invalid response status!");
                        current_interval = error_interval;
                    }
                }
                Err(e) => {
                    warn!("Failed to send Sentry CRON request with error: {e:#?}");
                    current_interval = error_interval;
                }
            }

            // Use a shorter interval when there's an error.
            tokio::time::sleep(std::time::Duration::from_secs(current_interval)).await;
        }
    }
}
