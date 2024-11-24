use crate::config::Config;
use crate::{config, launcher};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, span, Level};

#[derive(Clone)]
pub struct State {
    pub config: Arc<Config>,
    pub sockets: Arc<Sockets>,
}

#[derive(Default)]
pub struct Sockets {
    inner: HashMap<String, Arc<launcher::Socket>>,
}

impl Sockets {
    pub async fn from_servers(servers: &HashMap<String, config::Server>) -> Sockets {
        let mut sockets = Sockets {
            inner: HashMap::new(),
        };

        for (id, server) in servers {
            let span = span!(Level::INFO, "Sockets::from_servers", id, api = server.api).entered();

            let socket = match launcher::Socket::new(&server.api).await {
                Ok(v) => v,
                Err(err) => {
                    error!("Error with connecting to socket: {}", err);

                    continue;
                }
            };

            let pair = launcher::types::request::restore_token::Pair {
                name: "checkServer".to_string(),
                value: server.token.clone(),
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
            span.exit();

            sockets.insert(id, socket)
        }

        sockets
    }

    pub fn insert(&mut self, id: impl Into<String>, socket: launcher::Socket) {
        self.inner.insert(id.into(), Arc::new(socket));
    }

    pub fn socket(&self, id: impl Into<String>) -> Option<Arc<launcher::Socket>> {
        self.inner.get(&id.into()).cloned()
    }
}
