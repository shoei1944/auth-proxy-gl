use auth_proxy_gl::config::Config as AppConfig;
use auth_proxy_gl::state::Sockets;
use auth_proxy_gl::{routes, state};
use figment::providers;
use figment::providers::Format;
use std::error::Error;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{net, runtime, signal};
use tracing::{debug, info};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

const CONFIG_FILE: &str = "config.json";

fn main() -> Result<(), Box<dyn Error>> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var("LOGGING_LEVEL")
        .from_env()?;
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    if !fs::exists(CONFIG_FILE)? {
        let default_config = serde_json::to_string_pretty(&AppConfig::default())?;

        fs::write(CONFIG_FILE, default_config)?;

        return Ok(());
    }

    let config = figment::Figment::new()
        .join(providers::Json::file(CONFIG_FILE))
        .extract::<AppConfig>()?;

    let rt = runtime::Builder::new_multi_thread().enable_all().build()?;

    rt.block_on(_main(config))
}

async fn _main(config: AppConfig) -> Result<(), Box<dyn Error>> {
    let addr = format!("{}:{}", config.api.host, config.api.port).parse::<SocketAddr>()?;
    let listener = net::TcpListener::bind(addr).await?;
    info!("Proxy listening on address {}", addr);

    let sockets = Arc::new(Sockets::from_servers(&config.servers).await);

    let router = axum::Router::new()
        .nest(
            "/:server_id",
            axum::Router::new()
                .merge(routes::root::router())
                .nest("/api", routes::api::router())
                .nest("/sessionserver", routes::sessionserver::router()),
        )
        .with_state(state::State {
            config: Arc::new(config),
            sockets: sockets.clone(),
        });

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            signal::ctrl_c().await.unwrap();

            sockets.inner().for_each(|socket| socket.shutdown());

            debug!("Ctrl^C signal received. Quitting.");
        })
        .await?;

    Ok(())
}
