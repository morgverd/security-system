mod pushover;

use async_trait::async_trait;
use anyhow::{anyhow, Result};
use log::{debug, warn};
use crate::alerts::AlertInfo;
use crate::communications::pushover::PushoverCommunicationProvider;
use crate::config::EnvConfig;

#[async_trait]
pub(crate) trait CommunicationProvider: Send + Sync + 'static {

    /// Returns the provider name for logging.
    fn name() -> &'static str
    where
        Self: Sized;

    /// Creates a new communication provider instance with given configuration.
    /// Implementations can override this for custom initialization.
    /// If None is returned, the provider is invalid / misconfigured and cannot be used.
    fn from_config(config: &EnvConfig) -> Option<Self>
    where
        Self: Sized;

    /// Send the alert via provider.
    async fn send(&mut self, alert: &AlertInfo) -> Result<()>;
}

fn try_from_config<T: CommunicationProvider>(config: &EnvConfig) -> Option<(&'static str, Box<dyn CommunicationProvider>)> {
    match T::from_config(config) {
        Some(provider) => {
            debug!("Successfully created CommunicationProvider {}.", T::name());
            Some((T::name(), Box::new(provider) as Box<dyn CommunicationProvider>))
        },
        None => {
            warn!("CommunicationProvider {} has invalid configuration! Failed to initialize.", T::name());
            None
        }
    }
}

pub(crate) type CommunicationRegistryResult = (&'static str, Result<()>);

pub(crate) struct CommunicationRegistry {
    providers: Vec<(&'static str, Box<dyn CommunicationProvider>)>,
    size: usize
}
impl CommunicationRegistry {
    pub fn new(config: &EnvConfig) -> Result<Self> {

        // Attempt to create each provider from_config.
        let providers: Vec<_> = vec![
            try_from_config::<PushoverCommunicationProvider>(config)
        ]
        .into_iter()
        .flatten()
        .collect();

        let size = providers.len();
        if size == 0 {
            return Err(anyhow!("Failed to create any CommunicationProviders!"));
        }
        Ok(Self { providers, size })
    }

    /// Broadcast an alert across all registered providers.
    pub async fn broadcast(&mut self, alert: &AlertInfo) -> Vec<CommunicationRegistryResult> {
        let mut results = Vec::with_capacity(self.size);

        for (name, provider) in &mut self.providers {
            let result = provider.send(alert).await;
            results.push((*name, result));
        }

        results
    }
}