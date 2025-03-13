use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use async_trait::async_trait;
use anyhow::{anyhow, Result};
use log::{error, warn};
use reqwest::Client;
use crate::alerts::AlertInfo;
use crate::communications::{CommunicationProvider, CommunicationProviderResult};
use crate::config::EnvConfig;

/*
    TextAnywhere SMS Provider. An older provider that we already had credit with.
    This requires an internet connection, eventually the PI should have an SMS hat!
    https://www.textanywhere.com/
 */

const TEXT_ANYWHERE_URL: &str = "https://ws.textanywhere.net/HTTPRX/SendSMSEx.aspx";

pub(crate) struct TextAnywhereCommunicationProvider {
    client: Client,
    originator: String,
    username: String,
    password: String,
    destinations: String,
    counter: Arc<AtomicUsize>
}

impl TextAnywhereCommunicationProvider {
    fn create_send_form(&self, body: String) -> HashMap<&'static str, String> {
        let mut form = HashMap::new();
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);

        form.insert("Client_ID", self.username.clone());
        form.insert("Client_Pass", self.password.clone());
        form.insert("Originator", self.originator.clone());
        form.insert("Client_Ref", format!("alarm-{counter}"));
        form.insert("Billing_Ref", "security-alarm".to_string());
        form.insert("Connection", "1".to_string()); // 1 for testing, 2 for normal.
        form.insert("OType", "1".to_string());
        form.insert("DestinationEx", self.destinations.clone());
        form.insert("Body", body);
        form.insert("SMS_Type", "0".to_string());
        form.insert("Reply_Type", "0".to_string());

        form
    }

    fn parse_result(text: &String) -> Vec<(String, String)> {
         text.split(",")
            .filter_map(|v| {
                let mut parts = v.split(":");
                Some((parts.next()?.to_string(), parts.next()?.to_string()))
            })
            .collect()
    }
}

#[async_trait]
impl CommunicationProvider for TextAnywhereCommunicationProvider {
    fn name() -> &'static str { "text_anywhere" }

    fn from_config(config: &EnvConfig) -> Option<Self>
    where
        Self: Sized
    {
        if let (
            Some(originator),
            Some(username),
            Some(password),
            Some(destinations)
        ) = (
            &config.text_anywhere_originator,
            &config.text_anywhere_username,
            &config.text_anywhere_password,
            &config.text_anywhere_destinations
        ) {
            Some(Self {
                client: Client::new(),
                originator: originator.clone(),
                username: username.clone(),
                password: password.clone(),
                destinations: destinations.clone(),
                counter: Arc::new(AtomicUsize::new(0))
            })
        } else {
            None
        }
    }

    async fn send(&self, alert: &AlertInfo) -> Result<CommunicationProviderResult> {
        let body = alert.to_string();
        if body.len() > 160 {
            return Ok(CommunicationProviderResult::Invalid("SMS message body is too large!"))
        }

        let form = &self.create_send_form(body);
        let response = self.client.post(TEXT_ANYWHERE_URL)
            .form(form)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(anyhow!("TextAnywhere returned invalid status, expected 200 got {}!", status.to_string()));
        }

        let text = response.text().await?;
        let results = Self::parse_result(&text);

        let mut success = false;
        for (phone_number, status) in results {
            if status == "1" {
                success = true;
            } else {
                warn!("TextAnywhere destination '{}': {}", phone_number, status);
            }
        }

        // If all statuses are errors, then somehow the message or account must be invalid
        // so there is no point in resending it since we'll be met with more errors.
        if !success {
            error!("Failed to send any TextAnywhere messages! {}", text);
            return Ok(CommunicationProviderResult::Invalid("SMS message failed to send!"));
        }
        Ok(CommunicationProviderResult::Sent)
    }
}

