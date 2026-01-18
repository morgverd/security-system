use crate::alerts::initialize_alert_manager;
use crate::config::AppConfig;
use crate::monitors::spawn_monitors;
use crate::webhooks::get_routes;
use anyhow::{Context, Result};
use dotenv::dotenv;
use futures::future::select_all;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::signal::ctrl_c;
use tokio::sync::oneshot;

mod alerts;
mod config;
mod monitors;
mod webhooks;

fn main() -> Result<()> {
    dotenv().ok();

    // TODO: Make into clap cli argument.
    let config = AppConfig::load(Some("config.toml".into()))?;

    let mut log_builder = env_logger::Builder::new();
    log_builder.parse_env(env_logger::Env::default());

    let _guard = if let Some(ref sentry_dsn) = config.sentry.sentry_dsn {
        info!("Initializing Sentry...");

        // Ensure Sentry can capture error logs.
        let logger = sentry_log::SentryLogger::with_dest(log_builder.build());
        log::set_boxed_logger(Box::new(logger))
            .context("Failed to set Sentry logger as boxed logger!")?;
        log::set_max_level(log::LevelFilter::Trace);

        let panic_integration = sentry_panic::PanicIntegration::default().add_extractor(|_| None);
        Some(sentry::init((
            sentry_dsn.clone(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                integrations: vec![Arc::new(panic_integration)],
                before_send: Some(Arc::new(|event| {
                    warn!(
                        "Sending to Sentry: {}",
                        event
                            .message
                            .as_deref()
                            .or_else(|| {
                                event
                                    .exception
                                    .values
                                    .iter()
                                    .filter_map(|e| e.value.as_deref())
                                    .next()
                            })
                            .unwrap_or("Unknown!")
                    );
                    Some(event)
                })),
                ..Default::default()
            },
        )))
    } else {
        // Initialize default logger.
        let logger = log_builder.build();
        log::set_boxed_logger(Box::new(logger))
            .context("Failed to set non Sentry logger as boxed logger!")?;
        log::set_max_level(log::LevelFilter::Trace);
        warn!("Sentry DSN is unset! Not initializing.");
        None
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            // Create alarm manager task with shutdown signals.
            let (alarm_shutdown_tx, alarm_shutdown_rx) = oneshot::channel::<()>();
            let manager = initialize_alert_manager(&config)
                .await
                .expect("Failed to initialize AlertManager!");
            let manager_handle = tokio::spawn(async move {
                tokio::select! {
                    _ = manager.run() => warn!("AlertManager stopped unexpectedly."),
                   _ = alarm_shutdown_rx => {}
                }
            });

            // Create Warp HTTP server task with shutdown signals.
            let (warp_shutdown_tx, warp_shutdown_rx) = oneshot::channel::<()>();
            let warp_handle = tokio::spawn(async move {
                let (addr, server) = warp::serve(get_routes()).bind_with_graceful_shutdown(
                    config.server.http_addr,
                    async move {
                        let _ = warp_shutdown_rx.await;
                    },
                );

                info!("HTTP server listening on {addr}");
                server.await;
            });

            // If there are monitors, create and join them.
            let ctrl_c = ctrl_c();
            let monitor_handles = spawn_monitors(&config.monitors).await;
            if !monitor_handles.is_empty() {
                debug!("Joining with {} monitor handle(s)!", monitor_handles.len());
                tokio::select! {
                    _ = select_all(monitor_handles) => warn!("A monitor has stopped unexpectedly!"),
                    _ = ctrl_c => warn!("Received shutdown signal!")
                }
            } else {
                debug!("There are no monitor handles!");
                let _ = ctrl_c.await;
                warn!("Received shutdown signal!");
            }

            // Send shutdown signals.
            info!("Shutting down services...");
            let _ = alarm_shutdown_tx.send(());
            let _ = warp_shutdown_tx.send(());

            // Wait for tasks to terminate gracefully.
            let _ = manager_handle.await;
            let _ = warp_handle.await;
        });

    info!("Finished!");
    Ok(())
}
