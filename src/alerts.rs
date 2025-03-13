use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Instant, Duration, sleep};
use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use crate::communications::{CommunicationProviderResult, CommunicationRegistry};
use crate::config::EnvConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum AlertLevel {
    Info,
    Warning,
    Critical,
    Alarm
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
}
impl Display for AlertInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.source, self.message)
    }
}

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
    pub alarm_last: Arc<RwLock<Option<Instant>>>,
    pub alarm_cooldown: Duration,
    pub retry_max: u64,
    pub retry_delay: Duration,
    pub communications: Arc<CommunicationRegistry>
}
impl AlertWorker {
    pub async fn handle(&mut self, alert: AlertInfo) -> Result<()> {
        match alert.level {
            AlertLevel::Alarm => self.alarm(&alert).await,
            _ => self.broadcast(&alert).await,
        }
    }

    async fn alarm(&mut self, alert: &AlertInfo) -> Result<()> {
        let now = Instant::now();
        if let Some(alarm_last) = *self.alarm_last.read().await {

            // If the alarm is in a cooldown period, log but don't process the new alarm.
            if now.duration_since(alarm_last) < self.alarm_cooldown {
                warn!("Alarm suppressed during cooldown: {}", alert);
                return Ok(());
            }
        }

        info!("Entering an alarm state: {}", alert);
        *self.alarm_last.write().await = Some(now);
        loop {
            match self.broadcast(&alert).await {
                Ok(()) => break Ok(()),
                Err(e) => {

                    // If the broadcast fails to send, wait and try again.
                    debug!("Failed to broadcast Alarm event with error: {:#?}", e);
                    sleep(self.retry_delay).await;
                }
            }
        }
    }

    async fn broadcast(&mut self, alert: &AlertInfo) -> Result<()> {
        let providers_len = self.communications.len();
        let mut ignored_providers = HashSet::with_capacity(providers_len);
        let mut successes = 0;

        for attempt in 1..=self.retry_max {
            if attempt > 1 {
                debug!(
                    "Alert broadcast attempt {}/{}. Sleeping for {} seconds.",
                    attempt, self.retry_max, self.retry_delay.as_secs()
                );
                sleep(self.retry_delay).await;
            }

            let broadcast_results = self.communications.broadcast(&alert, &ignored_providers).await;
            let mut any_failed = false;

            for (name, result) in broadcast_results.into_iter() {
                match result {
                    Ok(CommunicationProviderResult::Sent) => {
                        debug!("Successfully sent alert via {}!", name);
                        ignored_providers.insert(name);
                        successes += 1;
                    },
                    Ok(CommunicationProviderResult::Invalid(e)) => {
                        warn!("Alert is invalid for provider {} with error: {}", name, e);
                        ignored_providers.insert(name);
                    },
                    Err(e) => {
                        warn!("Failed to send alert via {} with error: {}", name, e.to_string());
                        any_failed = true;
                    }
                }
            }

            if ignored_providers.len() == providers_len {
                return if successes == 0 {
                    Err(anyhow!("Failed to send communication to any provider as all reported it as invalid!"))
                } else {
                    Ok(())
                }
            }
            if !any_failed {
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

async fn save_state(alert: &AlertInfo, state_path: &PathBuf) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(state_path)
        .await?;
    let data = bincode::serialize(alert)?;
    file.write_all(&*data).await.map_err(anyhow::Error::from)
}

async fn delete_state(state_path: &PathBuf) -> Result<()> {
    if state_path.exists() {
        fs::remove_file(state_path).await?;
    }
    Ok(())
}

pub(crate) struct AlertManager {
    alarm_cooldown: Duration,
    retry_delay: Duration,
    retry_max: u64,
    states_dir_path: PathBuf,

    alarm_last: Arc<RwLock<Option<Instant>>>,
    communications: Arc<CommunicationRegistry>,

    semaphore: Arc<Semaphore>,
    counter: Arc<AtomicUsize>,
    receiver: mpsc::Receiver<AlertInfo>
}
impl AlertManager {
    pub fn new(config: &EnvConfig, communications: CommunicationRegistry) -> (Self, AlertSender) {
        let (sender, receiver) = mpsc::channel::<AlertInfo>(100);
        (
            Self {
                alarm_cooldown: Duration::from_secs(config.alarm_cooldown),
                retry_delay: Duration::from_secs(config.alerts_retry_delay),
                retry_max: config.alerts_retry_max,
                states_dir_path: config.alerts_states_dir.clone(),

                alarm_last: Arc::new(RwLock::new(None)),
                communications: Arc::new(communications),

                semaphore: Arc::new(Semaphore::new(config.alerts_concurrency_limit)),
                counter: Arc::new(AtomicUsize::new(0)),
                receiver
            },
            AlertSender { sender }
        )
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("Processing pending alerts...");
        for (pending, state_path) in self.load_existing_states().await?.into_iter() {
            self.execute(pending, state_path).await;
        }

        debug!("AlertManager starting to process channel alerts...");
        while let Some(alert) = self.receiver.recv().await {
            let state_path = self.create_state_path();
            if let Err(e) = save_state(&alert, &state_path).await {
                warn!("Failed to save alert state {} with error: {:#?}", state_path.to_str().unwrap_or("Unknown"), e);
            }
            self.execute(alert, state_path).await;
        }

        Err(anyhow!("Alert channel closed, no more alerts can be processed!"))
    }

    async fn execute(&self, alert: AlertInfo, state_path: PathBuf) {

        let mut worker = self.create_worker();
        let semaphore = Arc::clone(&self.semaphore);

        tokio::spawn(async move {
            let permit = semaphore.acquire().await.expect("Failed to acquire semaphore!");
            match worker.handle(alert).await {
                Ok(()) => debug!("Successfully processed alert!"),
                Err(e) => error!("Failed to process alert: {:#?}", e)
            }

            // TODO: Maybe only delete the state on success?
            if let Err(e) = delete_state(&state_path).await {
                warn!("Failed to delete alert state {} with error: {:#?}", state_path.to_str().unwrap_or("Unknown"), e);
            }
            drop(permit);
        });
    }

    async fn load_existing_states(&self) -> Result<Vec<(AlertInfo, PathBuf)>> {
        let mut dir_entries = fs::read_dir(&self.states_dir_path).await.context("Failed to read states directory!")?;
        let mut alerts: Vec<_> = vec![];

        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                match Self::load_state_file(&path).await {
                    Ok(alert) => alerts.push((alert, path.clone())),
                    Err(e) => warn!("Failed to load existing state with error: {:#?}", e)
                }
            }
        }

        // Ensure the worker counter starts after the last stored to prevent overwriting.
        let size = alerts.len();
        debug!("Loaded {} existing alert states!", size);
        if size > 0 {
            self.counter.fetch_add(size + 1, Ordering::SeqCst);
        }
        Ok(alerts)
    }

    async fn load_state_file(path: &PathBuf) -> Result<AlertInfo> {
        let mut file = fs::File::open(&path).await.context(format!("Failed to open file: {:?}", path))?;
        let mut contents = vec![];

        file.read_to_end(&mut contents).await.context(format!("Failed to read file: {:?}", path))?;
        bincode::deserialize(&contents).context(format!("Failed to deserialize AlertInfo binary data: {:?}", path))
    }

    fn create_state_path(&self) -> PathBuf {
        let unique_id = self.counter.fetch_add(1, Ordering::SeqCst);
        self.states_dir_path.join(format!("{}.bin", unique_id))
    }

    fn create_worker(&self) -> AlertWorker {
        AlertWorker {
            alarm_cooldown: self.alarm_cooldown,
            retry_delay: self.retry_delay,
            retry_max: self.retry_max,
            alarm_last: Arc::clone(&self.alarm_last),
            communications: Arc::clone(&self.communications)
        }
    }
}

lazy_static! {
    static ref ALERT_SENDER: Arc<Mutex<Option<AlertSender>>> = Arc::new(Mutex::new(None));
}

pub async fn initialize_alert_manager(config: &EnvConfig) -> Result<AlertManager> {
    let registry = CommunicationRegistry::new(&config)
        .context("Failed to initialize communication registry!")?;

    let (manager, sender) = AlertManager::new(&config, registry);
    {
        let mut lock = ALERT_SENDER.lock().await;
        *lock = Some(sender);
    }

    debug!("Initialized AlertManger with a valid communication registry!");
    Ok(manager)
}

/// Send an alert to the AlertManager via channel.
pub async fn send_alert(alert: AlertInfo) -> Result<()> {
    let lock = ALERT_SENDER.lock().await;
    let sender = lock.as_ref()
        .ok_or_else(|| anyhow!("AlertSender is not initialized!"))?;

    sender.send(alert).await
}