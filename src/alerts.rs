use crate::communications::{CommunicationProviderResult, CommunicationRegistry};
use crate::config::AppConfig;
use anyhow::{anyhow, Context, Result};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, OnceCell, RwLock, Semaphore};
use tokio::time::{sleep, Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum AlertLevel {
    Info,
    Warning,
    Critical,
    Alarm,
}
impl AlertLevel {
    pub fn as_u8(&self) -> u8 {
        match self {
            AlertLevel::Info => 1,
            AlertLevel::Warning => 2,
            AlertLevel::Critical => 3,
            AlertLevel::Alarm => 4,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AlertInfo {
    pub source: String,
    pub message: String,
    pub level: AlertLevel,
    pub timestamp: Option<u64>,
}
impl AlertInfo {
    pub fn new(source: String, message: String, level: AlertLevel) -> Result<Self> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;
        Ok(Self {
            source,
            message,
            level,
            timestamp: Some(timestamp.as_secs()),
        })
    }

    #[inline]
    pub fn is_alarm(&self) -> bool {
        self.level == AlertLevel::Alarm
    }
}
impl Display for AlertInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.source, self.message)
    }
}

#[derive(Clone)]
pub(crate) struct AlertSender {
    sender: mpsc::Sender<AlertInfo>,
}
impl AlertSender {
    pub async fn send(&self, alert: AlertInfo) -> Result<()> {
        self.sender
            .send(alert)
            .await
            .map_err(|_| anyhow!("Failed to queue alert; channel may be closed."))
    }
}

struct AlertWorker {
    pub retry_max: u64,
    pub retry_delay: Duration,
    pub communications: Arc<CommunicationRegistry>,
}
impl AlertWorker {
    async fn handle(&mut self, alert: &AlertInfo) -> Result<()> {
        let providers_len = self.communications.len();
        let mut ignored_providers = HashSet::with_capacity(providers_len);
        let mut successes = 0;

        for attempt in 1..=self.retry_max {
            if attempt > 1 {
                debug!(
                    "Alert broadcast {:?} attempt {}/{}. Sleeping for {} seconds.",
                    alert,
                    attempt,
                    self.retry_max,
                    self.retry_delay.as_secs()
                );
                sleep(self.retry_delay).await;
            }

            let broadcast_results = self
                .communications
                .broadcast(&alert, &ignored_providers)
                .await;
            let mut any_failed = false;

            for (name, result) in broadcast_results.into_iter() {
                match result {
                    Ok(CommunicationProviderResult::Sent) => {
                        debug!("Successfully sent alert via {}!", name);
                        ignored_providers.insert(name);
                        successes += 1;
                    }
                    Ok(CommunicationProviderResult::Invalid(e)) => {
                        warn!("Alert is invalid for provider {} with error: {}", name, e);
                        ignored_providers.insert(name);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to send alert via {} with error: {}",
                            name,
                            e.to_string()
                        );
                        any_failed = true;
                    }
                }
            }

            if ignored_providers.len() == providers_len {
                return if successes == 0 {
                    Err(anyhow!("Failed to send communication to any provider as all reported it as invalid!"))
                } else {
                    Ok(())
                };
            }
            if !any_failed {
                debug!(
                    "No communication providers failed, meaning all were successful or none ran."
                );
                return Ok(());
            }
        }

        if successes == 0 {
            Err(anyhow!(
                "Failed to send communication to any providers after {} attempts!",
                self.retry_max
            ))
        } else {
            warn!(
                "Failed to send communication to all providers after {} attempts, but did send to {}/{}.",
                self.retry_max,
                successes,
                providers_len
            );
            Ok(())
        }
    }
}

pub(crate) struct AlertManager {
    alarm_cooldown: Duration,
    alarm_last: Arc<RwLock<Option<Instant>>>,

    retry_delay: Duration,
    retry_max: u64,

    communications: Arc<CommunicationRegistry>,
    semaphore: Arc<Semaphore>,
    receiver: mpsc::Receiver<AlertInfo>,
}
impl AlertManager {
    pub fn new(config: &AppConfig) -> Result<(Self, AlertSender)> {
        let registry = CommunicationRegistry::new(&config.communications)
            .context("Failed to initialize communication registry!")?;

        let (sender, receiver) = mpsc::channel::<AlertInfo>(100);
        Ok((
            Self {
                alarm_cooldown: Duration::from_secs(config.alerts.alarm_cooldown),
                alarm_last: Arc::new(RwLock::new(None)),

                retry_delay: Duration::from_secs(config.alerts.send_retry_delay),
                retry_max: config.alerts.send_retry_max,

                communications: Arc::new(registry),
                semaphore: Arc::new(Semaphore::new(config.alerts.send_concurrency_limit)),
                receiver,
            },
            AlertSender { sender },
        ))
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("AlertManager starting to process channel alerts...");
        while let Some(alert) = self.receiver.recv().await {
            self.execute(alert).await;
        }

        Err(anyhow!(
            "Alert channel closed, no more alerts can be processed!"
        ))
    }

    async fn execute(&self, alert: AlertInfo) {
        let is_alarm = alert.is_alarm();
        if is_alarm {
            let mut alarm_last_guard = self.alarm_last.write().await;
            let now = Instant::now();

            if let Some(last) = *alarm_last_guard {
                if now.duration_since(last) < self.alarm_cooldown {
                    warn!("Alarm suppressed during cooldown: {alert}");
                    return;
                }
            }

            *alarm_last_guard = Some(now);
        }

        let mut worker = self.create_worker();
        let permit = if is_alarm {
            None
        } else {
            self.semaphore.clone().acquire_owned().await.ok()
        };

        tokio::spawn(async move {
            let _permit = permit;

            debug!("Executing alert: {alert:?}");
            match worker.handle(&alert).await {
                Ok(()) => debug!("Successfully processed alert!"),
                Err(e) => error!("Failed to process alert: {e:#?}"),
            }
        });
    }

    fn create_worker(&self) -> AlertWorker {
        AlertWorker {
            retry_delay: self.retry_delay,
            retry_max: self.retry_max,
            communications: self.communications.clone(),
        }
    }
}

static ALERT_SENDER: OnceCell<AlertSender> = OnceCell::const_new();

pub async fn initialize_alert_manager(config: &AppConfig) -> Result<AlertManager> {
    let (manager, sender) = AlertManager::new(config)?;
    ALERT_SENDER
        .set(sender)
        .map_err(|_| anyhow!("AlertSender already initialized!"))?;

    Ok(manager)
}

pub async fn send_alert(alert: AlertInfo) -> Result<()> {
    ALERT_SENDER
        .get()
        .ok_or_else(|| anyhow!("AlertSender is not initialized!"))?
        .send(alert)
        .await
}
