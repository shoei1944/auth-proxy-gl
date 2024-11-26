use crate::{config, launcher};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct State {
    pub key_pair: Arc<KeyPair>,
    pub servers: Arc<HashMap<String, config::Server>>,
    pub sockets: Arc<Sockets>,
}

pub struct KeyPair {
    pub public: String,
    pub private: String,
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
            sockets.insert(id, launcher::Socket::new(&server.api))
        }

        sockets
    }

    pub fn insert(&mut self, id: impl Into<String>, socket: launcher::Socket) {
        self.inner.insert(id.into(), Arc::new(socket));
    }

    pub fn socket(&self, id: impl Into<String>) -> Option<Arc<launcher::Socket>> {
        self.inner.get(&id.into()).cloned()
    }

    pub fn inner(&self) -> impl Iterator<Item = &Arc<launcher::Socket>> {
        self.inner.values()
    }
}
