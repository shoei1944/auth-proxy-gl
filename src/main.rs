use anyhow::Context;
use auth_proxy_gl::{args, config, config::Config as AppConfig, routes, state, state::Sockets};
use clap::Parser;
use figment::{providers, providers::Format};
use std::{error::Error, fs, net::SocketAddr, sync::Arc};
use tokio::{net, runtime, signal};
use tracing::{debug, info};
use tracing_subscriber::{
    filter::LevelFilter,
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

fn main() -> Result<(), Box<dyn Error>> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var("LOGGING_LEVEL")
        .from_env()?;
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    let args = args::Args::parse();

    if !fs::exists(&args.config_path)? {
        info!("Config file not found. Saving default.");

        let default_config = serde_json::to_string_pretty(&config::default())?;

        fs::write(&args.config_path, default_config)?;

        return Ok(());
    }

    let config = figment::Figment::new()
        .join(providers::Json::file(&args.config_path))
        .extract::<AppConfig>()?;

    let rt = runtime::Builder::new_multi_thread().enable_all().build()?;

    rt.block_on(_main(config))
}

async fn _main(config: AppConfig) -> Result<(), Box<dyn Error>> {
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
            servers: Arc::new(config.servers),
            key_pair: Arc::new(state::KeyPair {
                public: tokio::fs::read_to_string(&config.keys.public)
                    .await
                    .with_context(|| {
                        format!("failed to read public key from {:?}", config.keys.public)
                    })?,
                private: tokio::fs::read_to_string(&config.keys.private)
                    .await
                    .with_context(|| {
                        format!("failed to read private key from {:?}", config.keys.private)
                    })?,
            }),
            sockets: sockets.clone(),
        });

    let addr = format!("{}:{}", config.api.host, config.api.port).parse::<SocketAddr>()?;
    let listener = net::TcpListener::bind(addr).await?;
    info!("Proxy listening on address {}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            signal::ctrl_c().await.unwrap();

            sockets.inner().for_each(|socket| socket.shutdown());

            debug!("Ctrl^C signal received. Quitting.");
        })
        .await?;

    Ok(())
}
