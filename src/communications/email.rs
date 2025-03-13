use std::time::Duration;
use async_trait::async_trait;
use anyhow::Result;
use tokio::time::sleep;
use crate::alerts::AlertInfo;
use crate::communications::{CommunicationProvider, CommunicationProviderResult};
use crate::config::EnvConfig;

/*
    Pushover Communication Provider.
    Should take an application token, and a comma seperated list of user tokens to send notifications to.
 */

pub(crate) struct EmailCommunicationProvider { }

#[async_trait]
impl CommunicationProvider for EmailCommunicationProvider {
    fn name() -> &'static str { "email" }

    fn from_config(_: &EnvConfig) -> Option<Self>
    where
        Self: Sized
    {
        Some(Self {})
    }

    async fn send(&self, _: &AlertInfo) -> Result<CommunicationProviderResult> {
        sleep(Duration::from_secs(10)).await;
        Ok(CommunicationProviderResult::Sent)
    }
}