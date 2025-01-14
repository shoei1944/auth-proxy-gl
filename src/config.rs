use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub api: Api,
    pub keys: KeyPairPaths,
    pub servers: HashMap<String, Server>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Api {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KeyPairPaths {
    pub private: PathBuf,
    pub public: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Server {
    pub api: url::Url,
    pub token: String,
}

pub fn default() -> Config {
    Config {
        api: Api {
            host: "0.0.0.0".to_string(),
            port: 10000,
        },
        keys: KeyPairPaths {
            private: "private.pem".into(),
            public: "public.pem".into(),
        },
        servers: HashMap::new(),
    }
}
