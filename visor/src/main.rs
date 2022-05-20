mod config;
mod container;
mod instance;
mod notify;
mod psutil;
mod wechat;

use bollard::Docker;
use clap::Parser;
use log::info;
use wechat::wechat::Wechat;

use crate::config::Config;
use crate::container::container::*;
use crate::instance::*;
use crate::notify::WechatNotifier;
use crate::psutil::*;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_name = "FILE", default_value_t = String::from("config.json"))]
    config: String,

    /// Run as daemon
    #[clap(short)]
    daemon: bool,
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let args = Args::parse();
    let cfg = Config::new(&args.config).unwrap();

    let notifier = WechatNotifier::new(&cfg.notify_webhook).unwrap();
    let docker = Docker::connect_with_socket_defaults().unwrap();

    let mut wechat = Wechat::new(
        &cfg.wechat.corp_id,
        &cfg.wechat.app_secret,
        cfg.wechat.users.clone(),
    )
    .unwrap();
    wechat
        .map_users_by_department(cfg.wechat.department_id)
        .await
        .unwrap();
    info!("Wechat users count: {}", wechat.users.len());

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
