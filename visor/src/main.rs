mod config;
mod docker;
mod instance;
mod notify;
mod psutil;

use std::time::Duration;

use anyhow::Ok;
use anyhow::Result;
use clap::{Arg, Command};
use log::{info, warn};
use shiplift::Docker;

use crate::config::Config;
use crate::docker::*;
use crate::instance::*;
use crate::notify::{message_tpl, Notifier, WechatNotifier};
use crate::psutil::*;

async fn stop_containers<T: Notifier>(docker: &Docker, cfg: &Config, notifier: &T) -> Result<()> {
    loop {
        let cpu_usage = get_cpu_usage()?;
        let mem_usage = get_mem_usage()?;
        let disk_usage = get_disk_usage()?;
        info!(
            "CPU: {}%, MEM: {}%, DISK: {}%s",
            cpu_usage, mem_usage, disk_usage
        );
        if cpu_usage < cfg.cpu_limit && mem_usage < cfg.mem_limit {
            break;
        }

        let mut containers = list_running_containers(docker).await?;
        if containers.is_empty() {
            info!("No running containers found");
            return Ok(());
        }

        containers.sort_by(|a, b| {
            let a_time = status_into_running_time(a.status.clone()).unwrap_or_default();
            let b_time = status_into_running_time(b.status.clone()).unwrap_or_default();
            b_time.cmp(&a_time)
        });

        let container = &containers[0];
        let container_id = &container.id;
        let inst = get_instance(&container).unwrap_or_else(|e| {
            warn!("Get instance owner failed: {}", e);
            Instance::new()
        });
        stop_container(docker, container_id).await?;
        info!("Stop container: {}", container_id);

        let msg = message_tpl(container, &inst, &cfg.serv_url);
        notifier.notify(&msg).await?;
    }
    Ok(())
}

async fn clean_containers(docker: &Docker, cfg: &Config) -> Result<()> {
    let containers = list_exited_containers(docker).await?;
    if containers.is_empty() {
        info!("No exited containers found");
        return Ok(());
    }

    let interval = 60 * 60 * 24 * cfg.container_clean_interval;
    for container in containers.iter() {
        let t = status_into_running_time(container.status.clone()).unwrap_or_default();
        if t.lt(&Duration::from_secs(interval)) {
            info!("Container {} exited: {}s", &container.id, t.as_secs());
            continue;
        }
        remove_container(docker, &container.id).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let app = Command::new("visor")
        .version("0.1.7")
        .author("K8sCat <rustpanic@gmail.com>")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .default_value("config.yml")
                .help("Set config file")
                .takes_value(true),
        );
    let matches = app.get_matches();

    let cfg_path = matches.value_of("config").unwrap();
    let cfg = Config::new(cfg_path).unwrap();

    let notifier = WechatNotifier::new(cfg.notify_webhook.clone()).unwrap();
    let docker = Docker::new();

    // 限制 CPU 和内存使用率，并停止过载的容器
    if let Err(e) = stop_containers(&docker, &cfg, &notifier).await {
        warn!("Stop containers failed: {}", e);
    }

    // 清理部署目录
    if let Err(e) = clean_pkg(cfg.pkg_clean_interval) {
        warn!("Clean pkg failed: {:?}", e);
    };

    // 清理部署包
    if let Err(e) = clean_release(cfg.release_clean_interval) {
        warn!("Clean release failed: {:?}", e);
    };

    // 清理停止的容器
    if let Err(e) = clean_containers(&docker, &cfg).await {
        warn!("Clean containers failed: {}", e);
    };

    // 清理镜像
    if let Err(e) = clean_images(&docker).await {
        warn!("Clean images failed: {:?}", e);
    }

    // 清理数据卷
    if let Err(e) = clean_volumes(&docker).await {
        warn!("Clean volumes failed: {:?}", e);
    }
}
