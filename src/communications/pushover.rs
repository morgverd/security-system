use crate::alerts::{AlertInfo, AlertLevel};
use crate::communications::{CommunicationProvider, CommunicationProviderResult};
use crate::config::{CommunicationRecipient, CommunicationsConfig, PushoverCommunicationConfig};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::join_all;
use log::warn;
use reqwest::Client;
use serde::Serialize;

/*
   Pushover Communication Provider. Should take an application token,
   and a comma seperated list of user tokens to send notifications to.
   https://pushover.net/
*/

const PUSHOVER_URL: &str = "https://api.pushover.net/1/messages.json";

#[derive(Serialize)]
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
    client: Client,
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
        PushoverPayload {
            token: self.config.token.clone(),
            user: recipient.id.clone(),
            title: alert.source.clone(),
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

#[async_trait]
impl CommunicationProvider for PushoverCommunicationProvider {
    fn name() -> &'static str {
        "pushover"
    }

    fn from_config(config: &CommunicationsConfig) -> Option<Self>
    where
        Self: Sized,
    {
        if let Some(pushover) = &config.pushover {
            if pushover.recipients.is_empty() {
                warn!("Pushover recipients is empty!");
                return None;
            }

            Some(Self {
                client: Client::new(),
                config: pushover.clone(),
            })
        } else {
            None
        }
    }

    async fn send(&self, alert: &AlertInfo) -> Result<CommunicationProviderResult> {
        // TODO: implement level filtering, unify with monitors
        let futures = self.config.recipients.iter().map(|user| {
            let payload = self.create_payload(&user, &alert);
            self.client
                .post(PUSHOVER_URL)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(&payload)
                .send()
        });

        let (mut got_response, mut got_success) = (false, false);
        for result in join_all(futures).await {
            match result {
                Ok(response) => {
                    got_response = true;
                    let status = response.status();
                    if status.is_success() {
                        got_success = true;
                    } else {
                        warn!(
                            "Got invalid status back from Pushover, expected 200 got {}!",
                            status.to_string()
                        );
                    }
                }
                Err(e) => warn!("Failed to send Pushover message: {:?}", e),
            }
        }

        match (got_response, got_success) {
            (true, false) => Ok(CommunicationProviderResult::Invalid(
                "Pushover notification failed to send!",
            )),
            (_, true) => Ok(CommunicationProviderResult::Sent),
            _ => Err(anyhow!("Failed to send any Pushover notifications!")),
        }
    }
}
