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

const CONFIG_FILE: &str = "config.json";

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt().init();

    if !fs::exists(CONFIG_FILE)? {
        let default_config = serde_json::to_string_pretty(&AppConfig::default())?;

        fs::write(CONFIG_FILE, default_config)?;
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

    let sockets = Sockets::from_servers(&config.servers).await;

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
            sockets: Arc::new(sockets),
        });

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            signal::ctrl_c().await.unwrap();

            println!("Ctrl^C signal received. Quitting.");
        })
        .await?;

    Ok(())
}
