mod config;
mod docker;
mod instance;
mod notify;
mod psutil;

use std::time::Duration;

use anyhow::Ok;
use anyhow::Result;
use clap::Parser;
use log::{info, warn};
use shiplift::Docker;

use crate::config::Config;
use crate::docker::*;
use crate::instance::*;
use crate::notify::{message_tpl, Notifier, WechatNotifier};
use crate::psutil::*;

async fn stop_containers<T>(docker: &Docker, cfg: &Config, notifier: &T) -> Result<()>
where
    T: Notifier,
{
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
        let instance = get_instance(&container).unwrap_or_else(|e| {
            warn!("Get instance owner failed: {}", e);
            Instance::default()
        });
        stop_container(docker, container_id).await?;
        info!("Stop container: {}", container_id);

        let msg = message_tpl(container, &instance, &cfg.serv_url);
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

        if let Err(e) = remove_container(docker, &container.id).await {
            warn!("Remove container {} failed: {}", container.id, e);
        } else {
            info!("Removed container {}", &container.id);
        }
    }
    Ok(())
}

/// Monitor and clean resource usage.
#[derive(Parser, Debug)]
#[clap(author = "K8sCat <rustpanic@gmail.com>", version = "0.1.12", about, long_about = None)]
struct Args {
    #[clap(short, long, value_name = "FILE", default_value_t = String::from("config.json"))]
    config: String,

    /// Run as daemon
    #[clap(short)]
    daemon: bool,
}

async fn monitor<T>(cfg: &Config, docker: &Docker, notifier: &T) -> Result<()>
where
    T: Notifier,
{
    // 限制 CPU 和内存使用率，并停止过载的容器
    if let Err(e) = stop_containers(docker, cfg, notifier).await {
        warn!("Stop containers failed: {}", e);
    }

    // 清理部署目录
    if let Err(e) = clean_pkg(cfg.pkg_clean_interval, docker).await {
        warn!("Clean pkg failed: {}", e);
    };

    // 清理部署包
    if let Err(e) = clean_release(cfg.release_clean_interval) {
        warn!("Clean release failed: {}", e);
    };

    // 清理停止的容器
    if let Err(e) = clean_containers(&docker, &cfg).await {
        warn!("Clean containers failed: {}", e);
    };

    // 清理镜像
    if let Err(e) = clean_images(&docker).await {
        warn!("Clean images failed: {}", e);
    }

    // 清理数据卷
    if let Err(e) = clean_volumes(&docker).await {
        warn!("Clean volumes failed: {}", e);
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let args = Args::parse();

    let cfg = Config::new(&args.config).unwrap();

    let notifier = WechatNotifier::new(&cfg.notify_webhook).unwrap();
    let docker = Docker::new();

    if args.daemon {
        loop {
            monitor(&cfg, &docker, &notifier).await.unwrap();
        }
    }

    monitor(&cfg, &docker, &notifier).await.unwrap();
}
