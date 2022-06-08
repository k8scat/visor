use std::{collections::{HashMap, HashSet}, fs};

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
    pub whitelist: Whitelist,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Whitelist {
    pub containers: Option<Vec<String>>,
    pub containers_map: Option<HashSet<String>>,
    pub images: Option<Vec<String>>,
    pub images_map: Option<HashSet<String>>,
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
        
        let mut containers_map = HashSet::<String>::new();
        if let Some(containers) = &config.whitelist.containers {
            for id in containers.iter() {
                containers_map.insert(id.clone());
            }
        }
        config.whitelist.containers_map = Some(containers_map);

        let mut images_map = HashSet::<String>::new();
        if let Some(images) = &config.whitelist.images {
            for id in images.iter() {
                images_map.insert(id.clone());
            }
        }
        config.whitelist.images_map = Some(images_map);
        Ok(config)
    }
}
