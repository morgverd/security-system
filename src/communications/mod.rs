mod pushover;
mod sms;

use crate::alerts::AlertInfo;
use crate::communications::pushover::PushoverCommunicationProvider;
use crate::communications::sms::SMSCommunicationProvider;
use crate::config::{CommunicationRecipient, CommunicationsConfig};
use log::{debug, error, warn};

pub enum CommunicationSendResultKind {
    Completed { failed: Vec<usize> },
    Unavailable { reason: String },
}

#[async_trait::async_trait]
pub(crate) trait CommunicationProvider: Send + Sync + 'static {
    /// Returns the provider name for logging.
    fn name() -> &'static str
    where
        Self: Sized;

    /// Creates a new communication provider instance with given configuration.
    /// Implementations can override this for custom initialization.
    /// If None is returned, the provider is invalid / misconfigured and cannot be used.
    fn from_config(config: &CommunicationsConfig) -> anyhow::Result<Self>
    where
        Self: Sized;

    /// Get all recipients for communication provider.
    fn get_all_recipients(&self) -> &Vec<CommunicationRecipient>;

    /// Get all target recipients for the alert level.
    fn get_recipients(&self, alert: &AlertInfo) -> Vec<usize> {
        let level_u8 = u8::from(&alert.level);
        self.get_all_recipients()
            .iter()
            .enumerate()
            .filter(|(_, recipient)| level_u8 >= recipient.level)
            .map(|(index, _)| index)
            .collect()
    }

    /// Send the alert via provider.
    async fn send(&self, alert: &AlertInfo, recipients: &[usize]) -> CommunicationSendResultKind;
}

fn try_from_config<T: CommunicationProvider>(
    config: &CommunicationsConfig,
) -> Option<(&'static str, Box<dyn CommunicationProvider>)> {
    let name = T::name();
    match T::from_config(config) {
        Ok(provider) => {
            debug!("Successfully created CommunicationProvider '{name}'.");
            Some((name, Box::new(provider) as Box<dyn CommunicationProvider>))
        }
        Err(e) => {
            warn!("CommunicationProvider '{name}' failed to initialize: {e:?}");
            None
        }
    }
}

pub(crate) struct CommunicationRegistry {
    providers:
        std::sync::Arc<std::collections::HashMap<&'static str, Box<dyn CommunicationProvider>>>,
    retry_max: u64,
    retry_delay: std::time::Duration,
}
impl CommunicationRegistry {
    pub fn new(config: &CommunicationsConfig) -> anyhow::Result<Self> {
        // Attempt to create each provider from_config.
        let providers_vec: Vec<_> = vec![
            try_from_config::<SMSCommunicationProvider>(config),
            try_from_config::<PushoverCommunicationProvider>(config),
        ]
        .into_iter()
        .flatten()
        .collect();

        let size = providers_vec.len();
        if size == 0 {
            anyhow::bail!("Failed to create any CommunicationProviders!");
        }

        let mut providers = std::collections::HashMap::with_capacity(size);
        for (name, provider) in providers_vec {
            providers.insert(name, provider);
        }

        Ok(Self {
            providers: std::sync::Arc::new(providers),
            retry_max: config.retry_max,
            retry_delay: std::time::Duration::from_secs(config.retry_delay),
        })
    }

    pub async fn broadcast(&self, alert: &AlertInfo) {
        let futures: Vec<_> = self
            .providers
            .iter()
            .map(|(name, provider)| self.send_with_retry(name, provider.as_ref(), alert))
            .collect();

        futures::future::join_all(futures).await;
    }

    async fn send_with_retry(
        &self,
        name: &'static str,
        provider: &dyn CommunicationProvider,
        alert: &AlertInfo,
    ) {
        let mut recipients = provider.get_recipients(alert);
        if recipients.is_empty() {
            debug!(
                "There are no recipients for '{}' with level {:?}",
                name, alert.level
            );
            return;
        }

        for attempt in 1..=self.retry_max + 1 {
            match provider.send(alert, &recipients).await {
                CommunicationSendResultKind::Completed { failed } if failed.is_empty() => {
                    debug!("Sent to all recipients of '{name}' in {attempt} attempt(s)!");
                    return;
                }
                CommunicationSendResultKind::Completed { failed } => {
                    debug!(
                        "Attempt #{} for '{}': {} recipients failed, retrying after {}s",
                        attempt,
                        name,
                        failed.len(),
                        self.retry_delay.as_secs()
                    );
                    recipients = failed;
                    tokio::time::sleep(self.retry_delay).await;
                }
                CommunicationSendResultKind::Unavailable { reason } => {
                    error!("CommunicationProvider '{name}' is unavailable: {reason}");
                    return;
                }
            }
        }

        error!(
            "{} met retry limit with {} recipients left unsent for {:?}!",
            name,
            recipients.len(),
            alert
        );
    }
}
