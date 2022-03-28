use std::fs;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub notify_webhook: String,
    pub cpu_limit: f32,
    pub mem_limit: f32,
    pub release_clean_interval: u64,
    pub pkg_clean_interval: u64,
    pub container_clean_interval: u64,
    pub serv_url: String,
}

impl Config {
    pub fn new(path: &str) -> Result<Config> {
        let s = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&s)?;
        Ok(config)
    }
}
