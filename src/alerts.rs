use crate::communications::CommunicationRegistry;
use crate::config::AppConfig;
use anyhow::Context;
use log::{debug, warn};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct AlertInfo {
    pub source: String,
    pub message: String,
    pub level: AlertLevel,
    pub timestamp: Option<u64>,
}
impl AlertInfo {
    pub fn new(source: String, message: String, level: AlertLevel) -> anyhow::Result<Self> {
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
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
impl std::fmt::Display for AlertInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.source, self.message)
    }
}

#[derive(Clone)]
pub(crate) struct AlertSender {
    sender: tokio::sync::mpsc::Sender<AlertInfo>,
}
impl AlertSender {
    pub async fn send(&self, alert: AlertInfo) -> anyhow::Result<()> {
        self.sender
            .send(alert)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to queue alert; channel may be closed."))
    }
}

pub(crate) struct AlertManager {
    alarm_cooldown: tokio::time::Duration,
    alarm_last: std::sync::Arc<tokio::sync::RwLock<Option<tokio::time::Instant>>>,
    communications: std::sync::Arc<CommunicationRegistry>,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
    receiver: tokio::sync::mpsc::Receiver<AlertInfo>,
}
impl AlertManager {
    pub fn new(config: &AppConfig) -> anyhow::Result<(Self, AlertSender)> {
        let registry = CommunicationRegistry::new(&config.communications)
            .context("Failed to initialize communication registry!")?;

        let (sender, receiver) = tokio::sync::mpsc::channel::<AlertInfo>(100);
        Ok((
            Self {
                alarm_cooldown: tokio::time::Duration::from_secs(config.alerts.alarm_cooldown),
                alarm_last: std::sync::Arc::new(tokio::sync::RwLock::new(None)),

                communications: std::sync::Arc::new(registry),
                semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(
                    config.alerts.send_concurrency_limit,
                )),
                receiver,
            },
            AlertSender { sender },
        ))
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        debug!("AlertManager starting to process channel alerts...");
        while let Some(alert) = self.receiver.recv().await {
            self.execute(alert).await;
        }

        Err(anyhow::anyhow!(
            "Alert channel closed, no more alerts can be processed!"
        ))
    }

    async fn execute(&self, alert: AlertInfo) {
        // Enforce a cooldown on alarms, since the CCTV system could report multiple
        // alarms within rapid succession if motion is detected on multiple cameras.
        let is_alarm = alert.is_alarm();
        if is_alarm {
            let mut alarm_last_guard = self.alarm_last.write().await;
            let now = tokio::time::Instant::now();

            if let Some(last) = *alarm_last_guard {
                if now.duration_since(last) < self.alarm_cooldown {
                    warn!("Alarm suppressed during cooldown: {alert}");
                    return;
                }
            }

            *alarm_last_guard = Some(now);
        }

        // Ignore concurrency limit for alarms.
        let permit = if is_alarm {
            None
        } else {
            self.semaphore.clone().acquire_owned().await.ok()
        };

        // Hold semaphore permit in the communication task.
        let communications = self.communications.clone();
        tokio::spawn(async move {
            let _permit = permit;

            debug!("Executing alert: {alert:?}");
            communications.broadcast(&alert).await;
        });
    }
}

static ALERT_SENDER: tokio::sync::OnceCell<AlertSender> = tokio::sync::OnceCell::const_new();

pub async fn initialize_alert_manager(config: &AppConfig) -> anyhow::Result<AlertManager> {
    let (manager, sender) = AlertManager::new(config)?;
    ALERT_SENDER
        .set(sender)
        .map_err(|_| anyhow::anyhow!("AlertSender already initialized!"))?;

    Ok(manager)
}

pub async fn send_alert(alert: AlertInfo) -> anyhow::Result<()> {
    ALERT_SENDER
        .get()
        .ok_or_else(|| anyhow::anyhow!("AlertSender is not initialized!"))?
        .send(alert)
        .await
}
