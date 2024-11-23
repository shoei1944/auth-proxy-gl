use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub api: Api,
    pub keys: KeyPair,
    pub servers: HashMap<String, Server>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Api {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct KeyPair {
    pub private: String,
    pub public: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Server {
    pub api: String,
    pub token: String,
}
