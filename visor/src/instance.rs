use std::collections::HashSet;
use std::fs;
use std::ops::Sub;
use std::process::Command;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use bollard::models::ContainerSummary;
use bollard::Docker;
use log::{info, warn};
use serde::Deserialize;

use crate::config::Config;
use crate::container::container::*;
use crate::notify::Notifier;
use crate::wechat::wechat::Wechat;

const OWNER_FILE: &str = ".owner_email";

#[derive(Debug, Default)]
pub struct Instance {
    pub owner: String,
    pub deploy_dir: String,
    pub config: InstanceConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct InstanceConfig {
    pub base_url: String,
    pub volume: String,
}

// Todo: 获取实例访问地址、容器数据卷
pub fn get_instance(container: &ContainerSummary) -> Result<Instance> {
    let mut https_port: i64 = 0;
    if let Some(ports) = &container.ports {
        for port in ports.iter() {
            if port.private_port.eq(&443) {
                match port.public_port {
                    Some(p) => https_port = p,
                    None => return Err(anyhow!("No public port found for 443")),
                }
            }
        }
    }

    let cmd = format!(
        "grep -B1 {} /data/ones/autodeploy/records.log | tail -n 2 | head -n 1",
        https_port
    );
    let deploy_dir = exec(&cmd)?;
    if deploy_dir.is_empty() {
        return Err(anyhow!("Deploy dir not found"));
    }

    let config = get_instance_config(&deploy_dir).unwrap_or_default();

    let cmd = format!("cat {}/{}", deploy_dir, OWNER_FILE);
    let owner = exec(&cmd)?;
    Ok(Instance {
        owner,
        deploy_dir,
        config,
    })
}

fn get_instance_config(deploy_dir: &str) -> Result<InstanceConfig> {
    let dirs: Vec<String> = fs::read_dir(deploy_dir.clone())?
        .filter(|entry| match entry {
            Ok(entry) => entry.path().is_dir(),
            _ => false,
        })
        .map(|entry| {
            if entry.is_err() {
                String::new()
            } else {
                entry
                    .unwrap()
                    .path()
                    .to_str()
                    .unwrap_or_default()
                    .to_string()
            }
        })
        .collect();

    for d in dirs {
        let config_file = format!("{}/config.json", d);
        let s = fs::read_to_string(&config_file);
        match s {
            Ok(s) => {
                let c = serde_json::from_str::<InstanceConfig>(&s);
                match c {
                    Ok(c) => {
                        return Ok(c);
                    }
                    Err(e) => {
                        warn!("Deserialize instance config failed: {}", e);
                        continue;
                    }
                }
            }
            Err(e) => {
                warn!("Read instance config file failed: {}", e);
                continue;
            }
        }
    }
    Err(anyhow!("Instance config not found"))
}

fn exec(cmd: &str) -> Result<String> {
    let output = Command::new("bash").arg("-c").arg(cmd).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow!(
            "Failed to execute cmd [{}]: {} {}",
            cmd,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

const PKG_DIR: &str = "/data/ones/pkg";
const RELEASE_DIR: &str = "/data/release";

pub fn filter_files(dir: &str, lifecycle: u64) -> Result<Vec<String>> {
    let t = SystemTime::now().sub(Duration::from_secs(lifecycle * 86400));
    let files = fs::read_dir(dir)?
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            let m = entry.metadata().unwrap();
            m.modified().unwrap().lt(&t)
        })
        .map(|entry| entry.path().to_str().unwrap().to_string())
        .collect();
    Ok(files)
}

pub fn clean_release(lifecycle: u64) -> Result<()> {
    let files = filter_files(RELEASE_DIR, lifecycle)?;
    if files.is_empty() {
        info!("No release files found");
    } else {
        for f in files.iter() {
            if let Err(e) = fs::remove_file(f) {
                warn!("Remove release {} failed: {}", f, e);
            } else {
                info!("Removed release: {}", f);
            }
        }
    }
    Ok(())
}

pub async fn clean_pkg(docker: &Docker, lifecycle: u64) -> Result<()> {
    let containers = list_running_containers(docker).await?;
    let mut m = HashSet::<String>::with_capacity(containers.len());
    containers.iter().for_each(|container| {
        let instance = get_instance(container).unwrap_or_default();
        m.insert(instance.deploy_dir);
    });
    drop(containers);

    let files = filter_files(PKG_DIR, lifecycle)?;
    if files.is_empty() {
        info!("No pkg files found");
    } else {
        for f in files.iter() {
            if !f.starts_with(PKG_DIR) {
                info!("Invalid pkg: {}", f);
                continue;
            }
            if m.contains(f) {
                info!("Ignore pkg: {}", f);
                continue;
            }

            if let Err(e) = fs::remove_dir_all(f) {
                warn!("Remove pkg {} failed: {}", f, e);
            } else {
                info!("Removed pkg: {}", f);
            }
        }
    }
    Ok(())
}

pub async fn monitor<'a, T>(
    cfg: &Config,
    docker: &Docker,
    notifier: &T,
    wechat: &mut Wechat<'a>,
) -> Result<()>
where
    T: Notifier,
{
    let existed_containers_map = map_existed_containers(docker).await?;

    // 限制 CPU 和内存使用率，并停止过载的容器
    if let Err(e) = stop_containers(docker, cfg, notifier, wechat).await {
        warn!("Stop containers failed: {}", e);
    }

    // 清理部署目录
    if let Err(e) = clean_pkg(docker, cfg.lifecycle.pkg).await {
        warn!("Clean pkg failed: {}", e);
    };

    // 清理部署包
    if let Err(e) = clean_release(cfg.lifecycle.release) {
        warn!("Clean release failed: {}", e);
    };

    // 清理停止的容器
    if let Err(e) = clean_exited_containers(docker, cfg.lifecycle.container, &existed_containers_map).await {
        warn!("Clean containers failed: {}", e);
    };

    // 清理镜像
    if let Err(e) = clean_images(docker, cfg).await {
        warn!("Clean images failed: {}", e);
    }

    // 清理数据卷
    if let Err(e) = clean_volumes(docker).await {
        warn!("Clean volumes failed: {}", e);
    }
    Ok(())
}
