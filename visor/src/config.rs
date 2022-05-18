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
    pub container_running: u64,
    pub pkg: u64,
    pub release: u64,
    pub image_created: u64,
}

impl Config {
    pub fn new(path: &str) -> Result<Config> {
        let s = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&s)?;
        Ok(config)
    }
}
