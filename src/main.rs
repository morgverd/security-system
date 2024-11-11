use anyhow::Result;
use env_logger::Env;
use crate::monitors::spawn_monitors;
use crate::webhooks::get_routes;

mod webhooks;
mod monitors;
mod alerts;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(Env::new().default_filter_or("info"));

    // Spawn all monitors.
    spawn_monitors().await;

    // Serve webhook routes.
    warp::serve(get_routes())
        .run(([127, 0, 0, 1], 9050))
        .await;

    Ok(())
}