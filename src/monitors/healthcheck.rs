use crate::config::MonitorsConfig;
use crate::monitors::Monitor;
use log::{debug, warn};

/*
   Send healthcheck request per interval.
*/

pub(crate) struct HealthcheckMonitor {
    client: reqwest::Client,
    url: String,
    interval: u64,
}

#[async_trait::async_trait]
impl Monitor for HealthcheckMonitor {
    fn name() -> &'static str {
        "healthcheck"
    }

    fn from_config(config: &MonitorsConfig) -> anyhow::Result<Self> {
        let url = config
            .healthcheck
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing healthcheck!"))?
            .clone();

        // TODO: Add timeout to client via builder. See PushoverCommunicationProvider.
        Ok(HealthcheckMonitor {
            client: reqwest::Client::new(),
            interval: config.healthcheck_interval,
            url,
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
                        warn!("Failed to send healthcheck with invalid response status!");
                        current_interval = error_interval;
                    }
                }
                Err(e) => {
                    warn!("Failed to send healthcheck with error: {e:#?}");
                    current_interval = error_interval;
                }
            }

            // Use a shorter interval when there's an error.
            tokio::time::sleep(std::time::Duration::from_secs(current_interval)).await;
        }
    }
}
