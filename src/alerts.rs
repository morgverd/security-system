use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Instant, Duration, sleep};
use anyhow::{anyhow, Result};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use sms_client::Client;
use sms_client::http::HttpClient;
use sms_client::types::sms::SmsOutgoingMessage;
use tokio::sync::{mpsc, OnceCell, RwLock, Semaphore};
use crate::config::AppConfig;

/// (MinAlertLevel, PhoneNumber)
pub type AlertRecipient = (u8, String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum AlertLevel {
    Info,
    Warning,
    Critical,
    Alarm
}
impl AlertLevel {
    pub fn as_u8(&self) -> u8 {
        match self {
            AlertLevel::Info => 1,
            AlertLevel::Warning => 2,
            AlertLevel::Critical => 3,
            AlertLevel::Alarm => 4
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AlertInfo {
    pub source: String,
    pub message: String,
    pub level: AlertLevel,
    pub timestamp: Option<u64>
}
impl AlertInfo {
    pub fn new(source: String, message: String, level: AlertLevel) -> Result<Self> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;
        Ok(Self { source, message, level, timestamp: Some(timestamp.as_secs()) })
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
    sender: mpsc::Sender<AlertInfo>
}
impl AlertSender {
    pub async fn send(&self, alert: AlertInfo) -> Result<()> {
        self.sender.send(alert).await
            .map_err(|_| anyhow!("Failed to queue alert; channel may be closed."))
    }
}

struct AlertWorker {
    pub retry_max: u64,
    pub retry_base_delay: Duration,
    pub retry_max_delay: Duration,
    pub sms_http_client: Arc<HttpClient>,
    pub sms_recipients: Arc<Vec<AlertRecipient>>,
}

impl AlertWorker {
    pub async fn handle(&self, alert: &AlertInfo) -> Result<()> {
        let mut pending_indices = self.recipients_for_level(alert.level.as_u8());

        if pending_indices.is_empty() {
            debug!("No recipients configured for alert level {:?}", alert.level);
            return Ok(());
        }

        let mut attempts = 0;
        loop {
            let (failed_indices, last_error) = self.send_to_recipients(&pending_indices, alert).await;
            if failed_indices.is_empty() {
                return Ok(());
            }

            attempts += 1;
            if attempts >= self.retry_max {
                return Err(anyhow!(
                    "Broadcast failed for {} recipient(s) after {} retries: {:#?}",
                    failed_indices.len(),
                    self.retry_max,
                    last_error
                ))
            }

            let delay = self.calculate_backoff(attempts);
            debug!(
                "Failed to send to {} recipient(s) (attempt {}/{}), retrying in {:?}",
                failed_indices.len(), attempts, self.retry_max, delay
            );
            sleep(delay).await;

            pending_indices = failed_indices;
        }
    }

    fn recipients_for_level(&self, alert_level: u8) -> Vec<usize> {
        self.sms_recipients
            .iter()
            .enumerate()
            .filter(|(_, (min_level, _))| *min_level <= alert_level)
            .map(|(i, _)| i)
            .collect()
    }

    async fn send_to_recipients(
        &self,
        indices: &[usize],
        alert: &AlertInfo,
    ) -> (Vec<usize>, Option<anyhow::Error>) {
        let mut failed_indices = Vec::new();
        let mut last_error = None;

        for &i in indices {
            let (_, recipient) = &self.sms_recipients[i];
            let message = SmsOutgoingMessage::simple_message(recipient, alert.to_string());

            match self.sms_http_client.send_sms(&message).await {
                Ok(response) => {
                    debug!("Sent message ID #{} to: {}", response.message_id, recipient);
                }
                Err(e) => {
                    warn!("Failed to send alert to {}: {:#?}", recipient, e);
                    last_error = Some(e.into());
                    failed_indices.push(i);
                }
            }
        }

        (failed_indices, last_error)
    }

    #[inline]
    fn calculate_backoff(&self, attempt: u64) -> Duration {
        let backoff = self.retry_base_delay.saturating_mul(1 << (attempt - 1).min(6));
        backoff.min(self.retry_max_delay)
    }
}

pub(crate) struct AlertManager {
    alarm_cooldown: Duration,
    alarm_last: Arc<RwLock<Option<Instant>>>,

    retry_base_delay: Duration,
    retry_max_delay: Duration,
    retry_max: u64,

    sms_http_client: Arc<HttpClient>,
    sms_recipients: Arc<Vec<AlertRecipient>>,

    semaphore: Arc<Semaphore>,
    receiver: mpsc::Receiver<AlertInfo>
}
impl AlertManager {
    pub fn new(config: &AppConfig, sms_client: Client) -> (Self, AlertSender) {
        let (sender, receiver) = mpsc::channel::<AlertInfo>(100);
        (
            Self {
                alarm_cooldown: Duration::from_secs(config.alarm_cooldown),
                alarm_last: Arc::new(RwLock::new(None)),

                retry_base_delay: Duration::from_secs(config.alerts_retry_base_delay),
                retry_max_delay: Duration::from_secs(config.alerts_retry_max_delay),
                retry_max: config.alerts_retry_max,

                sms_http_client: sms_client.http_arc().expect("Missing SMS HttpClient!"),
                sms_recipients: Arc::new(config.get_sms_recipients()),

                semaphore: Arc::new(Semaphore::new(config.alerts_concurrency_limit)),
                receiver
            },
            AlertSender { sender }
        )
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("AlertManager starting to process channel alerts...");
        while let Some(alert) = self.receiver.recv().await {
            self.execute(alert).await;
        }

        Err(anyhow!("Alert channel closed, no more alerts can be processed!"))
    }

    async fn execute(&self, alert: AlertInfo) {
        let is_alarm = alert.is_alarm();
        if is_alarm {
            let mut alarm_last_guard = self.alarm_last.write().await;
            let now = Instant::now();

            if let Some(last) = *alarm_last_guard {
                if now.duration_since(last) < self.alarm_cooldown {
                    warn!("Alarm suppressed during cooldown: {}", alert);
                    return;
                }
            }

            *alarm_last_guard = Some(now);
        }

        let worker = self.create_worker();
        let permit = if is_alarm {
            None
        } else {
            self.semaphore.clone().acquire_owned().await.ok()
        };

        tokio::spawn(async move {
            let _permit = permit;
            match worker.handle(&alert).await {
                Ok(()) => debug!("Successfully processed alert!"),
                Err(e) => error!("Failed to process alert: {:#?}", e)
            }
        });
    }

    fn create_worker(&self) -> AlertWorker {
        AlertWorker {
            retry_base_delay: self.retry_base_delay,
            retry_max_delay: self.retry_max_delay,
            retry_max: self.retry_max,
            sms_http_client: Arc::clone(&self.sms_http_client),
            sms_recipients: Arc::clone(&self.sms_recipients)
        }
    }
}

static ALERT_SENDER: OnceCell<AlertSender> = OnceCell::const_new();

pub async fn initialize_alert_manager(config: &AppConfig) -> Result<AlertManager> {
    let sms_client = Client::new(config.get_sms_config())?;
    let (manager, sender) = AlertManager::new(config, sms_client);

    ALERT_SENDER.set(sender)
        .map_err(|_| anyhow!("AlertSender already initialized!"))?;

    Ok(manager)
}

pub async fn send_alert(alert: AlertInfo) -> Result<()> {
    ALERT_SENDER.get()
        .ok_or_else(|| anyhow!("AlertSender is not initialized!"))?
        .send(alert)
        .await
}