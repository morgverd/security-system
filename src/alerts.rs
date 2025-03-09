use std::fmt::{Display, Formatter};
use std::sync::Arc;
use tokio::time::{Instant, Duration};
use anyhow::Result;
use lazy_static::lazy_static;
use log::{info, warn};
use tokio::sync::Mutex;

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
    retry_delay: Duration
}
impl Default for AlertManager {
    fn default() -> Self {
        Self {
            alarm_last: None,
            alarm_cooldown: Duration::from_secs(300),
            notification_retries: 3,
            retry_delay: Duration::from_secs(30)
        }
    }
}
impl AlertManager {

    /// Push an alert, sending the notification directly or triggering alarm.
    pub async fn handle_alert(&mut self, alert: AlertInfo) -> Result<()> {
        match alert.level {
            AlertLevel::Alarm => self.handle_alarm(alert).await,
            _ => self.send_notifications(alert).await
        }
    }

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
        self.send_notifications(alert).await
    }

    async fn send_notifications(&mut self, alert: AlertInfo) -> Result<()> {
        info!("Imagine a notification was sent here: {}", alert);
        Ok(())
    }
}

lazy_static! {
    static ref ALERT_MANAGER: Arc<Mutex<AlertManager>> = Arc::new(Mutex::new(AlertManager::default()));
}

/// Send an alert to the AlertManager.
pub async fn send_alert(alert: AlertInfo) -> Result<()> {
    let mut manager = ALERT_MANAGER.lock().await;
    manager.handle_alert(alert).await
}