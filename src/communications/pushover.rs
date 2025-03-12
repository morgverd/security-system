use async_trait::async_trait;
use anyhow::Result;
use crate::alerts::AlertInfo;
use crate::communications::CommunicationProvider;
use crate::config::EnvConfig;

/*
    Pushover Communication Provider.
    Should take an application token, and a comma seperated list of user tokens to send notifications to.
 */

pub(crate) struct PushoverCommunicationProvider {
    test: String
}

#[async_trait]
impl CommunicationProvider for PushoverCommunicationProvider {
    fn name() -> &'static str { "Pushover" }

    fn from_config(config: &EnvConfig) -> Option<Self>
    where
        Self: Sized
    {
        if let Some(str) = &config.pushover_test {
            Some(Self { test: str.clone() })
        } else {
            None
        }
    }

    async fn send(&mut self, _: &AlertInfo) -> Result<()> {
        println!("Pushover Test: {}", self.test);
        Ok(())
    }
}