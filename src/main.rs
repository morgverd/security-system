use std::sync::Arc;
use anyhow::{Context, Result};
use dotenv::dotenv;
use futures::future::select_all;
use log::{debug, error, info, warn};
use crate::config::from_env;
use crate::monitors::spawn_monitors;
use crate::webhooks::get_routes;

mod webhooks;
mod monitors;
mod alerts;
mod config;

fn main() -> Result<()> {

    dotenv().ok();
    let config = from_env()?;

    let mut log_builder = env_logger::Builder::new();
    log_builder.filter_level(config.log_level);
    log_builder.parse_env(env_logger::Env::default());

    let _guard = if let Some(ref sentry_dsn) = config.sentry_dsn {
        info!("Initializing Sentry...");

        // Ensure Sentry can capture error logs.
        let logger = sentry_log::SentryLogger::with_dest(log_builder.build());
        log::set_boxed_logger(Box::new(logger)).context("Failed to set Sentry logger as boxed logger!")?;
        log::set_max_level(log::LevelFilter::Trace);

        let panic_integration = sentry_panic::PanicIntegration::default().add_extractor(|_| None);
        Some(sentry::init((sentry_dsn.clone(), sentry::ClientOptions {
            release: sentry::release_name!(),
            integrations: vec![Arc::new(panic_integration)],
            before_send: Some(Arc::new(|event| {
                warn!(
                    "Sending to Sentry: {}",
                    event.message.as_deref().or_else(|| {
                        event.exception.values.iter()
                            .filter_map(|e| e.value.as_deref())
                            .next()
                    }).unwrap_or("Unknown!")
                );
                Some(event)
            })),
            ..Default::default()
        })))
    } else {

        // Initialize default logger.
        let logger = log_builder.build();
        log::set_boxed_logger(Box::new(logger)).context("Failed to set non Sentry logger as boxed logger!")?;
        log::set_max_level(log::LevelFilter::Trace);
        warn!("Sentry DSN is unset! Not initializing.");
        None
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {

            // Create monitors and HTTP handles.
            let monitor_handles = spawn_monitors(&config).await;
            let mut server_handle_opt = Some(tokio::spawn(async move {
                warp::serve(get_routes())
                    .run(config.server_addr)
                    .await;
            }));

            // If there are no monitors, just wait for the web server.
            if monitor_handles.is_empty() {

                debug!("Waiting only for server handle as there are no active monitors.");
                if let Some(server_handle) = server_handle_opt.take() {
                    server_handle.await.expect("Server handle panicked!");
                }
            } else {

                // Wait for either the webserver or any monitor to complete.
                debug!("Waiting for web server and {} active monitors.", monitor_handles.len());
                tokio::select! {
                    _ = &mut server_handle_opt.as_mut().unwrap() => error!("The web server has stopped!"),
                    res = select_all(monitor_handles) => {
                        let (result, index, remaining) = res;
                        error!("Monitor at index {index} {}!", if let Err(e) = result { format!("failed: {e}") } else { "completed unexpectedly".to_string() });

                        // Abort all remaining tasks.
                        for handle in remaining {
                            handle.abort();
                        }
                        if let Some(server_handle) = server_handle_opt.take() {
                            server_handle.abort();
                        }
                    }
                }
            }
        });

    info!("Shutting down...");
    Ok(())
}