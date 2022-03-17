use anyhow::{anyhow, Result};
use shiplift::rep::Container;
use std::process::Command;

const OWNER_FILE: &str = ".owner_email";

pub fn get_instance_owner(container: &Container) -> Result<String> {
    let mut https_port: u64 = 0;
    for port in container.ports.iter() {
        if port.private_port.eq(&443) {
            match port.public_port {
                Some(p) => https_port = p,
                None => return Err(anyhow!("No public port found for 443")),
            }
        }
    }

    let cmd = format!("grep -B1 {} /data/ones/autodeploy/records.log | head -n 1", https_port);
    let deploy_dir = exec(&cmd)?;
    let cmd = format!("cat {}/{}", deploy_dir, OWNER_FILE);
    let owner = exec(&cmd)?;
    Ok(owner)
}

fn exec(cmd: &str) -> Result<String> {
    let output = Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow!("Failed to execute cmd [{}]: {} {}", cmd, output.status, String::from_utf8_lossy(&output.stderr)))
    }
}
