use std::{collections::HashMap, fs};

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub notify_webhook: String,
    pub cpu_limit: f32,
    pub mem_limit: f32,
    pub serv_url: String,
    pub lifecycle: Lifecycle,
    pub wechat: Wechat,
    #[serde(default = "Vec::new")]
    pub whitelist: Vec<String>,
    #[serde(default = "HashMap::new")]
    pub whitelist_map: HashMap<String, ()>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Wechat {
    pub corp_id: String,
    pub app_secret: String,
    pub department_id: u32,
    pub users: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Lifecycle {
    pub container: u64,
    pub pkg: u64,
    pub release: u64,
    pub image_created: u64,
}

impl Config {
    pub fn new(path: &str) -> Result<Config> {
        let s = fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&s)?;
        if !config.whitelist.is_empty() {
            for id in config.whitelist.iter() {
                config.whitelist_map.insert(id.clone(), ());
            }
        }
        Ok(config)
    }
}
