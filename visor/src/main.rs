mod config;
mod docker;
mod instance;
mod notify;
mod psutil;
mod wechat;

use anyhow::Ok;
use anyhow::Result;
use clap::Parser;
use log::info;
use log::warn;
use shiplift::Docker;
use wechat::wechat::Wechat;

use crate::config::Config;
use crate::docker::*;
use crate::instance::*;
use crate::notify::{Notifier, WechatNotifier};
use crate::psutil::*;

/// Monitor resource usage and clean unused resource, keep the server usable.
#[derive(Parser, Debug)]
#[clap(author = "K8sCat <rustpanic@gmail.com>", version = "0.1.19", about, long_about = None)]
struct Args {
    #[clap(short, long, value_name = "FILE", default_value_t = String::from("config.json"))]
    config: String,

    /// Run as daemon
    #[clap(short)]
    daemon: bool,
}

async fn monitor<'a, T>(
    cfg: &Config,
    docker: &Docker,
    notifier: &T,
    wechat: &mut Wechat<'a>,
) -> Result<()>
where
    T: Notifier,
{
    let users = wechat
        .map_users_by_department(cfg.wechat.department_id)
        .await?;
    info!("Found users count: {}", users.len());

    // 限制 CPU 和内存使用率，并停止过载的容器
    if let Err(e) = stop_containers(docker, cfg, notifier, &users).await {
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
    if let Err(e) = clean_exited_containers(docker, cfg.lifecycle.container_running).await {
        warn!("Clean containers failed: {}", e);
    };

    // 清理镜像
    if let Err(e) = clean_images(docker, cfg.lifecycle.image_created).await {
        warn!("Clean images failed: {}", e);
    }

    // 清理数据卷
    if let Err(e) = clean_volumes(docker).await {
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

    let mut wechat = Wechat::new(&cfg.wechat.corp_id, &cfg.wechat.app_secret);

    if args.daemon {
        loop {
            monitor(&cfg, &docker, &notifier, &mut wechat)
                .await
                .unwrap();
        }
    }

    monitor(&cfg, &docker, &notifier, &mut wechat)
        .await
        .unwrap();
}
