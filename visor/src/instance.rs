use std::collections::HashMap;
/// 实例资源
use std::fs;
use std::ops::Sub;
use std::process::Command;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use log::{info, warn};
use shiplift::rep::Container;
use shiplift::Docker;

use crate::docker::list_running_containers;

const OWNER_FILE: &str = ".owner_email";

#[derive(Debug, Default)]
pub struct Instance {
    pub owner: String,
    pub deploy_dir: String,
}

pub fn get_instance(container: &Container) -> Result<Instance> {
    let mut https_port: u64 = 0;
    for port in container.ports.iter() {
        if port.private_port.eq(&443) {
            match port.public_port {
                Some(p) => https_port = p,
                None => return Err(anyhow!("No public port found for 443")),
            }
        }
    }

    let cmd = format!(
        "grep -B1 {} /data/ones/autodeploy/records.log | head -n 1",
        https_port
    );
    let deploy_dir = exec(&cmd)?;
    let cmd = format!("cat {}/{}", deploy_dir, OWNER_FILE);
    let owner = exec(&cmd)?;
    Ok(Instance { owner, deploy_dir })
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
    let mut m = HashMap::<String, bool>::with_capacity(containers.len());
    containers.iter().for_each(|container| {
        let instance = get_instance(container).unwrap_or_default();
        m.insert(instance.deploy_dir, true);
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

            if let None = m.get(f) {
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
