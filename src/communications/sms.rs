use crate::alerts::AlertInfo;
use crate::communications::{CommunicationProvider, CommunicationProviderResult};
use crate::config::{CommunicationRecipient, CommunicationsConfig, SMSCommunicationConfig};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::warn;
use sms_client::types::sms::SmsOutgoingMessage;

pub(crate) struct SMSCommunicationProvider {
    client: sms_client::Client,
    config: SMSCommunicationConfig,
}
impl SMSCommunicationProvider {
    fn create_message(
        &self,
        recipient: &CommunicationRecipient,
        alert: &AlertInfo,
    ) -> SmsOutgoingMessage {
        SmsOutgoingMessage::simple_message(recipient.id.clone(), alert.to_string())
    }
}

#[async_trait]
impl CommunicationProvider for SMSCommunicationProvider {
    fn name() -> &'static str {
        "sms"
    }

    fn from_config(config: &CommunicationsConfig) -> Option<Self>
    where
        Self: Sized,
    {
        if let Some(sms) = &config.sms {
            if sms.recipients.is_empty() {
                warn!("SMS recipients is empty!");
                return None;
            }

            Some(Self {
                client: sms_client::Client::new(sms.get_sms_config()).ok()?,
                config: sms.clone(),
            })
        } else {
            None
        }
    }

    async fn send(&self, alert: &AlertInfo) -> Result<CommunicationProviderResult> {
        let http = self.client.http().unwrap();

        // There is no point in using futures here since the SMS server queues operations anyway.
        let mut got_success = false;
        for recipient in self.config.recipients.iter() {
            if !recipient.is_target_level(recipient.level) {
                continue;
            }

            let message = self.create_message(recipient, alert);
            match http.send_sms(&message).await {
                Ok(_) => got_success = true,
                Err(e) => warn!("Failed to send SMS message: {:?}", e),
            }
        }

        if got_success {
            Ok(CommunicationProviderResult::Sent)
        } else {
            Err(anyhow!("Failed to send any SMS messages"))
        }
    }
}
