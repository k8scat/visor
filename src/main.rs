mod config;
mod container;
mod instance;
mod notify;
mod psutil;

use clap::{App, Arg};
use log::{info, warn};
use shiplift::Docker;

use crate::config::Config;
use crate::container::*;
use crate::instance::*;
use crate::notify::{message_tpl, Notifier};
use crate::psutil::*;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let app = App::new("visor")
        .version("0.1.0")
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

    loop {
        let cpu_usage = get_cpu_usage().unwrap();
        let mem_usage = get_mem_usage().unwrap();
        info!("CPU: {}%, MEM: {}%", cpu_usage, mem_usage);
        if cpu_usage > cfg.cpu_limit || mem_usage > cfg.mem_limit {
            let mut containers = list_containers(&docker).await.unwrap();
            if containers.is_empty() {
                info!("No containers found");
                continue;
            }
            containers.sort_by(|a, b| {
                let a_time = status_into_time(a.status.clone()).unwrap_or_default();
                let b_time = status_into_time(b.status.clone()).unwrap_or_default();
                b_time.cmp(&a_time)
            });
            let container = &containers[0];
            let container_id = &container.id;
            let inst = get_instance(&container).unwrap_or_else(|e| {
                warn!("Get instance owner failed: {}", e);
                Instance {
                    owner: String::from(""),
                    deploy_dir: String::from(""),
                }
            });
            stop_container(&docker, container_id).await.unwrap();
            notifier
                .notify(
                    message_tpl(container, &inst.owner, &cfg.serv_url, &inst.deploy_dir).as_str(),
                )
                .await
                .unwrap();
        }
    }
}
