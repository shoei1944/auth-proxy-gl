use auth_proxy_gl::config::Config as AppConfig;
use auth_proxy_gl::{launcher, routes, state};
use figment::providers;
use figment::providers::Format;
use std::error::Error;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{net, runtime, signal};
use tracing::{error, info, span, Level};

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

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(_main(config))
}

async fn _main(config: AppConfig) -> Result<(), Box<dyn Error>> {
    let addr = format!("{}:{}", config.api.host, config.api.port).parse::<SocketAddr>()?;
    let listener = net::TcpListener::bind(addr).await?;

    let sockets = connect_sockets(&config).await;

    let router = axum::Router::new()
        .nest(
            "/:server_id",
            axum::Router::new()
                .merge(routes::root::router())
                .nest("/api", routes::api::router())
                .nest(
                    "/sessionserver/session/minecraft",
                    routes::sessionserver_session_minecraft::router(),
                ),
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

async fn connect_sockets(config: &AppConfig) -> state::Sockets {
    let mut sockets = state::Sockets::default();

    for (server_id, data) in &config.servers {
        let servers_span = span!(
            Level::INFO,
            "connect_sockets ",
            id = server_id,
            api = data.api
        );
        let _enter = servers_span.enter();

        let socket = match launcher::Socket::new(&data.api).await {
            Ok(v) => v,
            Err(err) => {
                error!("Error with connecting to socket: {}", err);

                continue;
            }
        };

        let pair = launcher::types::request::restore_token::Pair {
            name: "checkServer".to_string(),
            value: data.token.clone(),
        };
        match socket.restore_token(pair, false).await {
            Ok(v) => {
                if !v.invalid_tokens.is_empty() {
                    error!("Invalid tokens received: {:?}", v.invalid_tokens);

                    continue;
                }
            }
            Err(err) => {
                error!("Error with send restore token request: {:?}", err);

                continue;
            }
        }

        info!("Connected");
        sockets.insert(server_id, socket)
    }

    sockets
}
