mod pushover;
mod sms;

use crate::alerts::AlertInfo;
use crate::communications::pushover::PushoverCommunicationProvider;
use crate::communications::sms::SMSCommunicationProvider;
use crate::config::CommunicationsConfig;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::join_all;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub(crate) enum CommunicationProviderResult {
    Sent,
    Invalid(&'static str),
}

#[async_trait]
pub(crate) trait CommunicationProvider: Send + Sync + 'static {
    /// Returns the provider name for logging.
    fn name() -> &'static str
    where
        Self: Sized;

    /// Creates a new communication provider instance with given configuration.
    /// Implementations can override this for custom initialization.
    /// If None is returned, the provider is invalid / misconfigured and cannot be used.
    fn from_config(config: &CommunicationsConfig) -> Option<Self>
    where
        Self: Sized;

    /// Send the alert via provider.
    async fn send(&self, alert: &AlertInfo) -> Result<CommunicationProviderResult>;
}

fn try_from_config<T: CommunicationProvider>(
    config: &CommunicationsConfig,
) -> Option<(&'static str, Box<dyn CommunicationProvider>)> {
    match T::from_config(config) {
        Some(provider) => {
            debug!("Successfully created CommunicationProvider {}.", T::name());
            Some((
                T::name(),
                Box::new(provider) as Box<dyn CommunicationProvider>,
            ))
        }
        None => {
            warn!(
                "CommunicationProvider {} has invalid configuration! Failed to initialize.",
                T::name()
            );
            None
        }
    }
}

pub(crate) struct CommunicationRegistry {
    providers: Arc<HashMap<&'static str, Box<dyn CommunicationProvider>>>,
    size: usize,
}
impl CommunicationRegistry {
    pub fn new(config: &CommunicationsConfig) -> Result<Self> {
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
            return Err(anyhow!("Failed to create any CommunicationProviders!"));
        }

        // Convert to HashMap for more efficient lookups.
        let mut providers = HashMap::with_capacity(size);
        for (name, provider) in providers_vec {
            providers.insert(name, provider);
        }

        Ok(Self {
            providers: Arc::new(providers),
            size,
        })
    }

    /// Broadcast an alert across all registered providers.
    /// Accepts a set of ignored providers to skip over for retries.
    pub async fn broadcast(
        &self,
        alert: &AlertInfo,
        ignored_providers: &HashSet<&'static str>,
    ) -> HashMap<&'static str, Result<CommunicationProviderResult>> {
        let ignored_empty = ignored_providers.is_empty();
        let mut futures = Vec::with_capacity(self.size);
        for (&name, provider) in self.providers.iter() {
            // If there are no ignored providers include all, otherwise skip ignored.
            if ignored_empty || !ignored_providers.contains(name) {
                futures.push(async move { (name, provider.send(alert).await) });
            }
        }

        let responses = join_all(futures).await;
        let mut results = HashMap::with_capacity(responses.len());
        for (name, result) in responses {
            results.insert(name, result);
        }

        results
    }

    /// Get the length of available providers set.
    pub fn len(&self) -> usize {
        self.size
    }
}
