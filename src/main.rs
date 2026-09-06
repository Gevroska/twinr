mod cache;
mod chat;
mod clips;
mod config;
mod errors;
mod ffmpeg;
mod hls;
mod media;
mod metadata;
mod proxy;
mod routes;
mod security;
mod state;
mod twitch;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let state = state::AppState::new(config::Config::from_env()?)?;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("listening on 0.0.0.0:3000");
    axum::serve(listener, routes::router(state)).await?;
    Ok(())
}
