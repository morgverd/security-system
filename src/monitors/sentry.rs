use crate::config::AppConfig;
use crate::monitors::Monitor;
use anyhow::Result;
use async_trait::async_trait;
use log::{debug, warn};
use reqwest::Client;
use std::cmp::max;
use std::time::Duration;
use tokio::time::sleep;

/*
   Send Sentry CRON requests per interval.
   This is used as remote health-checks for the system.
*/

pub(crate) struct SentryCronMonitor {
    url: String,
    interval: u64,
}

#[async_trait]
impl Monitor for SentryCronMonitor {
    fn name() -> &'static str {
        "sentry"
    }

    fn from_config(config: &AppConfig) -> Option<Self> {
        config
            .sentry_cron_url
            .as_ref()
            .map(|url| SentryCronMonitor {
                url: url.clone(),
                interval: config.sentry_cron_interval,
            })
    }

    async fn run(&mut self) -> Result<()> {
        let error_interval = max(self.interval / 2, 1);
        let client = Client::new();

        debug!("Started with an interval of {} seconds!", self.interval);
        loop {
            let mut current_interval = self.interval;
            match client.get(&self.url).send().await {
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
            sleep(Duration::from_secs(current_interval)).await;
        }
    }
}
