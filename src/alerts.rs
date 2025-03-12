use std::fmt::{Display, Formatter};
use std::sync::Arc;
use tokio::time::{Instant, Duration};
use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use log::{debug, info, warn};
use tokio::sync::Mutex;
use crate::communications::CommunicationRegistry;
use crate::config::EnvConfig;

#[derive(Debug, Clone)]
pub(crate) enum AlertLevel {
    Info,
    Warning,
    Critical,
    Alarm
}

pub struct AlertInfo {
    pub source: String,
    pub message: String,
    pub level: AlertLevel
}
impl Display for AlertInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.source, self.message)
    }
}

struct AlertManager {
    alarm_last: Option<Instant>,
    alarm_cooldown: Duration,
    notification_retries: u8,
    retry_delay: Duration,
    communications: CommunicationRegistry
}
impl AlertManager {
    pub fn new(communications: CommunicationRegistry) -> Self {
        Self {
            alarm_last: None,
            alarm_cooldown: Duration::from_secs(300),
            notification_retries: 3,
            retry_delay: Duration::from_secs(30),
            communications
        }
    }

    /// Push an alert, sending the notification directly or triggering alarm.
    pub async fn handle_alert(&mut self, alert: AlertInfo) -> Result<()> {
        match alert.level {
            AlertLevel::Alarm => self.handle_alarm(alert).await,
            _ => self.broadcast(alert).await
        }
    }

    /// Handle an alarm level alert with a cooldown between triggers.
    async fn handle_alarm(&mut self, alert: AlertInfo) -> Result<()> {

        let now = Instant::now();
        if let Some(alarm_last) = &self.alarm_last {

            // If the alarm is in a cooldown period, log but don't process the new alarm.
            if now.duration_since(*alarm_last) < self.alarm_cooldown {
                warn!("Alarm suppressed during cooldown: {}", alert);
                return Ok(());
            }
        }

        // Either not in an alarm state, or it's passed cooldown.
        info!("Entering an alarm state: {alert}");
        self.alarm_last = Some(now);
        self.broadcast(alert).await
    }

    /// Broadcast the alert via all communication providers in registry.
    async fn broadcast(&mut self, alert: AlertInfo) -> Result<()> {
        let broadcast_results = self.communications.broadcast(&alert).await;
        for (name, result) in broadcast_results.into_iter() {
            match result {
                Ok(()) => info!("Successfully sent alert via {name}!"),
                Err(e) => warn!("Failed to send alert via {name} with error: {e:#?}")
            }
        }

        Ok(())
    }
}

lazy_static! {
    static ref ALERT_MANAGER: Arc<Mutex<Option<AlertManager>>> = Arc::new(Mutex::new(None));
}

pub async fn initialize_alert_manager(config: &EnvConfig) -> Result<()> {
    let registry = CommunicationRegistry::new(config)
        .context("Failed to initialize communication registry!")?;

    // Once the manager is created, store it in ALERT_MANAGER lock to be used in send_alert.
    // TODO: Rewrite to use some AppState which contains the manager and is passed to monitors?
    let mut lock = ALERT_MANAGER.lock().await;
    *lock = Some(AlertManager::new(registry));

    debug!("Initialized AlertManger with a valid communication registry!");
    Ok(())
}

/// Send an alert to the AlertManager.
pub async fn send_alert(alert: AlertInfo) -> Result<()> {
    let mut lock = ALERT_MANAGER.lock().await;
    let manager = lock.as_mut()
        .ok_or_else(|| anyhow!("AlertManager is not initialized!"))?;

    manager.handle_alert(alert).await
}