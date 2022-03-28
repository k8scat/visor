use std::time::Duration;

use clap::{App, Arg};
use log::{info, warn};
use shiplift::Docker;

use crate::config::Config;
use crate::docker::*;
use crate::instance::*;
use crate::notify::{message_tpl, Notifier};
use crate::psutil::*;

mod config;
mod docker;
mod instance;
mod notify;
mod psutil;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let app = App::new("visor")
        .version("0.1.4")
        .author("K8sCat <rustpanic@gmail.com>")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .default_value("config.yml")
                .help("Set config file")
                .takes_value(true),
        );
    let matches = app.get_matches();

    let cfg_path = matches.value_of("config").unwrap();
    let cfg = Config::new(cfg_path).unwrap();

    let notifier = Notifier::new(cfg.notify_webhook.clone()).unwrap();
    let docker = Docker::new();

    let cpu_usage = get_cpu_usage().unwrap();
    let mem_usage = get_mem_usage().unwrap();
    let disk_usage = get_disk_usage().unwrap();
    info!("CPU: {}%, MEM: {}%, DISK: {}%s", cpu_usage, mem_usage, disk_usage);

    // 清理实例
    if cpu_usage > cfg.cpu_limit || mem_usage > cfg.mem_limit {
        let mut containers = list_running_containers(&docker).await.unwrap();
        if containers.is_empty() {
            info!("No running containers found");
        } else {
            containers.sort_by(|a, b| {
                let a_time = status_into_time(a.status.clone()).unwrap_or_default();
                let b_time = status_into_time(b.status.clone()).unwrap_or_default();
                b_time.cmp(&a_time)
            });

            let container = &containers[0];
            let container_id = &container.id;
            let inst = get_instance(&container).unwrap_or_else(|e| {
                warn!("Get instance owner failed: {}", e);
                Instance::new()
            });
            stop_container(&docker, container_id).await.unwrap();

            let msg = message_tpl(container, &inst, &cfg.serv_url);
            notifier.notify(&msg).await.unwrap();
        }
    }

    // 清理磁盘空间
    delete_pkg(cfg.pkg_clean_interval).unwrap();
    delete_release(cfg.release_clean_interval).unwrap();

    let containers = list_exited_containers(&docker).await.unwrap();
    if containers.is_empty() {
        info!("No exited containers found");
    } else {
        for container in containers.iter() {
            let container_id = &container.id;
            let t = status_into_time(container.status.clone()).unwrap_or_default();
            let interval = 60 * 60 * 24 * cfg.container_clean_interval;
            if t.gt(&Duration::from_secs(interval)) {
                remove_container(&docker, container_id).await.unwrap();
            } else {
                info!("Container {} exited: {}s", container_id, t.as_secs());
            }
        }
    }

    if let Err(e) = image_prune(&docker).await {
        warn!("Image prune failed: {}", e);
    }
    if let Err(e) = volume_prune(&docker).await {
        warn!("Volume prune failed: {}", e);
    }
}
