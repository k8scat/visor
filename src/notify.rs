use serde_json::json;
use anyhow::{Result, anyhow};
use shiplift::rep::Container;

// 群机器人配置说明 https://developer.work.weixin.qq.com/document/path/91770

#[derive(Debug, Clone)]
pub struct Notifier {
    webhook: String
}

impl Notifier {
    pub fn new(webhook: String) -> Result<Self> {
        if webhook.is_empty() {
            Err(anyhow!("Webhook is empty"))
        } else {
            Ok(Self { webhook })
        }
    }

    pub async fn notify(&self, message: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let body = json!({
            "msgtype": "markdown",
            "markdown": json!({
                "content": message
            })
        });
        let res = client.post(&self.webhook).json(&body).send().await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to send notification: {}", res.text().await?))
        }
    }
}

pub fn message_tpl(container: &Container, owner_email: &str, serv_url: &str) -> String {
    let mut container_id = container.id.clone();
    container_id.truncate(12);

    let mut running_time = container.status.clone();
    running_time = running_time.replace("Up ", "");
    running_time = running_time.replace(" (unhealthy)", "");
    running_time = running_time.replace(" (healthy)", "");
    running_time = running_time.replace(" (health: starting)", "");

    let start_container_url = format!("{}/start_container/{}", serv_url, container.id);

    format!(
        r##"由于私有部署环境资源使用达到上限，以下容器已被强制停止:
 > 容器ID: <font color="comment">{}</font>
 > 运行时长: <font color="comment">{}</font>
 > 创建者: <font color="comment">{}</font>


 如需继续使用该实例，可自行重启容器:
 > 重启命令: <font color="comment">docker start {}</font>
 > 重启链接: [Start Container]({})"##,
        container_id, running_time, owner_email, container_id, start_container_url
    )
}
