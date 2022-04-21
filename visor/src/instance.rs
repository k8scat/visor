/// 实例资源
use std::fs;
use std::ops::Sub;
use std::process::Command;
use std::time::Duration;
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use log::info;
use shiplift::rep::Container;

const OWNER_FILE: &str = ".owner_email";

#[derive(Debug, Clone)]
pub struct Instance {
    pub owner: String,
    pub deploy_dir: String,
}

impl Instance {
    pub fn new() -> Self {
        Self {
            owner: String::new(),
            deploy_dir: String::new(),
        }
    }
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

pub const PKG_DIR: &str = "/data/ones/pkg";
pub const RELEASE_DIR: &str = "/data/release";

pub fn filter_files(dir: &str, interval: u64) -> Result<Vec<String>> {
    let tl = SystemTime::now().sub(Duration::from_secs(interval * 86400));
    let files = fs::read_dir(dir)?
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            let m = entry.metadata().unwrap();
            m.modified().unwrap().lt(&tl)
        })
        .map(|entry| entry.path().to_str().unwrap().to_string())
        .collect();
    Ok(files)
}

pub fn clean_release(clean_interval: u64) -> Result<()> {
    let files = filter_files(RELEASE_DIR, clean_interval)?;
    if files.is_empty() {
        info!("No release files found");
    } else {
        for f in files.iter() {
            info!("Remove release: {}", f);
            fs::remove_file(f)?;
        }
    }
    Ok(())
}

pub fn clean_pkg(clean_interval: u64) -> Result<()> {
    let files = filter_files(PKG_DIR, clean_interval)?;
    if files.is_empty() {
        info!("No pkg files found");
    } else {
        for f in files.iter() {
            fs::read_dir(f)?
                .map(|entry| entry.unwrap())
                .for_each(|entry| {
                    let path = entry.path();
                    let p = path.to_str().unwrap();
                    if path.is_file()
                        && str::ends_with(path.to_str().unwrap_or_default(), ".tar.gz")
                    {
                        info!("Remove pkg: {}", p);
                        fs::remove_file(p).unwrap_or_else(|e| {
                            info!("Failed to remove pkg: {}, reason: {}", p, e);
                        });
                    } else {
                        info!("Skip: {}", p);
                    }
                });
        }
    }
    Ok(())
}
