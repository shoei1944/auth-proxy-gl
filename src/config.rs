use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub api: Api,
    pub keys: KeyPairPaths,
    pub servers: HashMap<String, Server>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Api {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct KeyPairPaths {
    pub private: PathBuf,
    pub public: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Server {
    pub api: String,
    pub token: String,
}
