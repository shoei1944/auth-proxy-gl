use auth_proxy_gl::config::Config as AppConfig;
use auth_proxy_gl::{api, launcher, state};
use config::Config;
use std::error::Error;
use std::fs;
use std::sync::Arc;
use tokio::{net, runtime, signal};
use tracing::{error, info};

const CONFIG_FILE: &str = "config.json";

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt().init();

    if fs::exists(CONFIG_FILE).map(|v| !v).unwrap_or(false) {
        let default_config = serde_json::to_string_pretty(&AppConfig::default())?;

        fs::write(CONFIG_FILE, default_config)?;
    }

    let config = Config::builder()
        .add_source(config::File::new(CONFIG_FILE, config::FileFormat::Json))
        .build()?
        .try_deserialize::<AppConfig>()?;

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(_main(config))
}

async fn _main(config: AppConfig) -> Result<(), Box<dyn Error>> {
    let listener =
        net::TcpListener::bind(format!("{}:{}", config.api.host, config.api.port)).await?;

    let sockets = connect_sockets(&config).await;

    let state = state::State {
        config: Arc::new(config),
        sockets: Arc::new(sockets),
    };

    let router = axum::Router::new()
        .nest("/:server_id", api::root::routes())
        .nest(
            "/:server_id/sessionserver/session/minecraft",
            api::sessions_server::routes(),
        )
        .with_state(state);

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            signal::ctrl_c().await.unwrap();

            println!("Ctrl^C signal received. Quitting.");
        })
        .await?;

    Ok(())
}

async fn connect_sockets(config: &AppConfig) -> state::Sockets {
    let mut sockets = state::Sockets::default();

    for (server_id, data) in &config.servers {
        let socket = match launcher::Socket::new(&data.api).await {
            Ok(v) => v,
            Err(err) => {
                error!(
                    "Error with connecting to socket ({} - {}): {}",
                    server_id, data.api, err
                );

                continue;
            }
        };

        let pair = launcher::types::request::restore_token::Pair {
            name: "checkServer".to_string(),
            value: data.token.clone(),
        };
        socket.restore_token(pair, false).await;

        info!("Connected ({} - {})", server_id, data.api);
        sockets.insert(server_id, socket)
    }

    sockets
}
