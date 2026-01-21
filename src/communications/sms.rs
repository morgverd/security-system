use crate::alerts::AlertInfo;
use crate::communications::{CommunicationProvider, CommunicationSendResultKind};
use crate::config::{CommunicationRecipient, CommunicationsConfig, SMSCommunicationConfig};

pub(crate) struct SMSCommunicationProvider {
    client: sms_client::Client,
    config: SMSCommunicationConfig,
}
impl SMSCommunicationProvider {
    fn create_message(
        &self,
        recipient: &CommunicationRecipient,
        alert: &AlertInfo,
    ) -> sms_client::types::sms::SmsOutgoingMessage {
        sms_client::types::sms::SmsOutgoingMessage::simple_message(
            recipient.target.clone(),
            format!("sentinel - {alert}"),
        )
    }
}

#[async_trait::async_trait]
impl CommunicationProvider for SMSCommunicationProvider {
    fn name() -> &'static str {
        "sms"
    }

    fn from_config(config: &CommunicationsConfig) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let config = match &config.sms {
            Some(config) => config,
            None => anyhow::bail!("Missing any SMS config!"),
        };

        Ok(Self {
            client: sms_client::Client::new(config.get_sms_config())
                .map_err(|e| anyhow::anyhow!(e))?,
            config: config.clone(),
        })
    }

    #[inline]
    fn get_all_recipients(&self) -> &Vec<CommunicationRecipient> {
        &self.config.recipients
    }

    async fn send(&self, alert: &AlertInfo, recipients: &[usize]) -> CommunicationSendResultKind {
        let http = match self.client.http() {
            Ok(http) => http,
            Err(_) => {
                return CommunicationSendResultKind::Unavailable {
                    reason: "Missing SMS HttpClient".to_string(),
                }
            }
        };

        // There is no point in using futures here since the SMS server queues operations anyway.
        let mut failed = Vec::with_capacity(recipients.len());
        for index in recipients.iter() {
            let message = self.create_message(&self.config.recipients[*index], alert);

            match http.send_sms(&message).await {
                Ok(_) => {}
                Err(_) => failed.push(*index),
            }
        }
        CommunicationSendResultKind::Completed { failed }
    }
}
