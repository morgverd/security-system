use crate::alerts::{AlertInfo, AlertLevel};
use crate::communications::{CommunicationProvider, CommunicationSendResultKind};
use crate::config::{CommunicationRecipient, CommunicationsConfig, PushoverCommunicationConfig};

/*
   Pushover Communication Provider.
   https://pushover.net/
*/

const PUSHOVER_URL: &str = "https://api.pushover.net/1/messages.json";

#[derive(serde::Serialize)]
struct PushoverPayload {
    pub token: String,
    pub user: String,
    pub title: String,
    pub message: String,
    pub priority: i8,
    pub retry: Option<u32>,
    pub expire: Option<u32>,
    pub timestamp: Option<u64>,
}

pub(crate) struct PushoverCommunicationProvider {
    client: reqwest::Client,
    config: PushoverCommunicationConfig,
}
impl PushoverCommunicationProvider {
    /// Create a payload to send to Pushover.
    fn create_payload(
        &self,
        recipient: &CommunicationRecipient,
        alert: &AlertInfo,
    ) -> PushoverPayload {
        let is_emergency = alert.level == AlertLevel::Alarm;

        // TODO: Reduce clones, maybe Arc<str>?
        PushoverPayload {
            token: self.config.token.clone(),
            user: recipient.target.clone(),
            title: format!("sentinel - {}", alert.source.clone()),
            message: alert.message.clone(),
            priority: match alert.level {
                AlertLevel::Info => -1,
                AlertLevel::Warning => 0,
                AlertLevel::Critical => 1,
                AlertLevel::Alarm => 2,
            },
            retry: if is_emergency { Some(30) } else { None },
            expire: if is_emergency { Some(1800) } else { None },
            timestamp: alert.timestamp,
        }
    }
}

#[async_trait::async_trait]
impl CommunicationProvider for PushoverCommunicationProvider {
    fn name() -> &'static str {
        "pushover"
    }

    fn from_config(config: &CommunicationsConfig) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let config = match &config.pushover {
            Some(config) => config,
            None => anyhow::bail!("Missing any Pushover config!"),
        };

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(config.timeout))
                .build()
                .unwrap_or_default(),
            config: config.clone(),
        })
    }

    #[inline]
    fn get_all_recipients(&self) -> &Vec<CommunicationRecipient> {
        &self.config.recipients
    }

    async fn send(&self, alert: &AlertInfo, recipients: &[usize]) -> CommunicationSendResultKind {
        // Create a request future for each recipient since Pushover can handle simultaneous requests.
        let futures = recipients.iter().map(|index| {
            let payload = self.create_payload(&self.config.recipients[*index], alert);

            async move {
                let result = self
                    .client
                    .post(PUSHOVER_URL)
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json")
                    .json(&payload)
                    .send()
                    .await;
                (index, result)
            }
        });

        // Join all futures, tracking each failed send.
        let mut failed = Vec::with_capacity(recipients.len());
        for (index, result) in futures::future::join_all(futures).await {
            match result {
                Ok(_) => {}
                Err(_) => failed.push(*index),
            }
        }
        CommunicationSendResultKind::Completed { failed }
    }
}
