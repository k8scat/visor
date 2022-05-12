use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct Message<'a> {
    msgtype: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    markdown: Option<&'a Markdown>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a Text>,
}

#[derive(Debug, Serialize)]
pub struct Text {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentioned_list: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentioned_mobile_list: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct Markdown {
    pub content: String,
}

impl<'a> Message<'a> {
    pub fn markdown(m: &'a Markdown) -> Self {
        Self {
            msgtype: MessageType::Markdown,
            markdown: Some(m),
            text: None,
        }
    }

    pub fn text(t: &'a Text) -> Self {
        Self {
            msgtype: MessageType::Text,
            markdown: None,
            text: Some(t),
        }
    }
}

#[derive(Debug, Serialize)]
pub enum MessageType {
    #[serde(rename(serialize = "markdown"))]
    Markdown,
    #[serde(rename(serialize = "text"))]
    Text,
}

pub struct GroupRobot {
    webhook: String,
}

#[derive(Debug, Deserialize)]
struct SendMessageResponse {
    errcode: i32,
    errmsg: String,
}

impl<'a> GroupRobot {
    pub fn new(webhook: String) -> Result<Self> {
        if webhook.is_empty() {
            Err(anyhow!("Webhook is empty"))
        } else {
            Ok(Self { webhook })
        }
    }

    pub async fn send_message(&self, message: &Message<'a>) -> Result<()> {
        let res = reqwest::Client::new()
            .post(&self.webhook)
            .json(message)
            .send()
            .await?
            .json::<SendMessageResponse>()
            .await?;
        if res.errcode == 0 {
            Ok(())
        } else {
            Err(anyhow!("Failed to send notification: {}", res.errmsg))
        }
    }
}
