use crate::{
    config::Config,
    wechat::group_robot::{GroupRobot, Markdown, Message, Text},
};
use anyhow::{anyhow, Ok, Result};
use async_trait::async_trait;
use bollard::models::ContainerSummary;

use crate::{get_cpu_usage, get_disk_usage, get_mem_usage, parse_status_time, Instance};

// 群机器人配置说明 https://developer.work.weixin.qq.com/document/path/91770

#[async_trait]
pub trait Notifier {
    async fn notify(&self, msg: &str, user_id: Option<&String>) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct WechatNotifier {
    webhook: String,
}

#[async_trait]
impl Notifier for WechatNotifier {
    async fn notify(&self, msg: &str, user_id: Option<&String>) -> Result<()> {
        let group = GroupRobot::new(self.webhook.clone())?;

        let m = Markdown {
            content: msg.to_string(),
        };
        let markdown = Message::markdown(&m);
        group.send_message(&markdown).await?;

        let mentioned_list = user_id.map(|id| vec![id.clone()]);
        let t = &Text {
            content: "".to_string(),
            mentioned_list,
            mentioned_mobile_list: None,
        };
        let text = Message::text(&t);
        group.send_message(&text).await?;
        Ok(())
    }
}

impl WechatNotifier {
    pub fn new(webhook: &str) -> Result<Self> {
        if webhook.is_empty() {
            Err(anyhow!("Webhook is empty"))
        } else {
            Ok(Self {
                webhook: webhook.to_string(),
            })
        }
    }
}

pub fn message_tpl(container: &ContainerSummary, inst: &Instance, cfg: &Config) -> String {
    let mut container_id = container.id.clone().unwrap_or_default();
    container_id.truncate(12);

    let items = parse_status_time(&container.status.clone().unwrap_or_default());
    let running_time = format!("{} {}", items[0], items[1]);

    let cpu_usage = get_cpu_usage().unwrap();
    let mem_usage = get_mem_usage().unwrap();
    let disk_usage = get_disk_usage().unwrap();
    let mut s = format!(
        r##"由于私有部署环境资源使用达到上限，以下容器已被强制停止:
> 访问地址: [{}]({})
> 创建者: <font color="comment">{}</font>
> 运行时长: <font color="comment">{}</font>
> 容器ID: <font color="comment">{}</font>
> 部署目录: <font color="comment">{}</font>
> 数据卷: <font color="comment>{}</font>

当前资源使用情况:
> CPU: <font color="comment">{}%</font>
> 内存: <font color="comment">{}%</font>
> 磁盘: <font color="comment">{}%</font>

如需继续使用该实例，可自行重启容器:
> 重启命令: <font color="comment">docker start {}</font>"##,
        inst.config.base_url,
        inst.config.base_url,
        inst.owner,
        running_time,
        container_id,
        inst.deploy_dir,
        inst.config.volume,
        cpu_usage as i32,
        mem_usage as i32,
        disk_usage as i32,
        container_id
    );

    if !cfg.serv_url.is_empty() {
        s = format!(
            r##"{}
> 重启链接: [点击启动]({})"##,
            s,
            format!("{}/start_container/{}", cfg.serv_url, container_id)
        )
    }

    format!(
        r##"{}

<font color="warning">注意: 实例如不再使用，将在 {} 天后被删除！</font>"##,
        s, cfg.lifecycle.container,
    )
}
