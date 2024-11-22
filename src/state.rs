use crate::config::Config;
use crate::launcher;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct State {
    pub config: Arc<Config>,
    pub sockets: Arc<Sockets>,
}

#[derive(Default)]
pub struct Sockets {
    inner: HashMap<String, Arc<Mutex<launcher::Socket>>>,
}

impl Sockets {
    pub fn insert(&mut self, id: impl Into<String>, socket: launcher::Socket) {
        self.inner.insert(id.into(), Arc::new(Mutex::new(socket)));
    }

    pub fn socket(&self, id: impl Into<String>) -> Option<Arc<Mutex<launcher::Socket>>> {
        self.inner.get(&id.into()).cloned()
    }
}
