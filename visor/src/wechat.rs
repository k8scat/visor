use chrono::{DateTime, Local, TimeZone};
use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;

const BASE_API: &str = "https://qyapi.weixin.qq.com";

#[derive(Debug)]
pub struct Wechat<'a> {
    pub corp_id: &'a str,
    pub app_secret: &'a str,
    pub access_token: Option<String>,
    pub expires_time: Option<DateTime<Local>>,
    pub client: reqwest::Client,
}

#[derive(Deserialize)]
pub struct AccessToken {
    pub errcode: u32,
    pub errmsg: String,
    pub access_token: String,
    pub expires_in: i64,
}

#[derive(Deserialize)]
pub struct User {
    pub userid: String,
    pub name: String,
    pub email: String,
}

#[derive(Deserialize)]
pub struct ListDetailUsersResponse {
    pub errcode: u32,
    pub errmsg: String,
    pub userlist: Vec<User>,
}

impl<'a> Wechat<'a> {
    pub fn new(corp_id: &'a str, app_secret: &'a str) -> Self {
        let client = reqwest::Client::new();
        Self {
            corp_id,
            app_secret,
            access_token: None,
            expires_time: None,
            client,
        }
    }

    pub async fn map_users_by_department(
        &mut self,
        department_id: u32,
    ) -> Result<HashMap<String, String>> {
        self.refresh_access_token().await?;
        if self.access_token.is_none() {
            return Err(anyhow!("access_token is None"));
        }

        let api = format!("{}/cgi-bin/user/list", BASE_API);
        let res = self
            .client
            .get(&api)
            .query(&[
                ("department_id", department_id.to_string()),
                ("fetch_child", "1".to_string()),
                ("access_token", self.access_token.clone().unwrap()),
            ])
            .send()
            .await?
            .json::<ListDetailUsersResponse>()
            .await?;
        if res.errcode != 0 {
            Err(anyhow!("list detail users failed: {}", res.errmsg))
        } else {
            let mut m = HashMap::new();
            for user in res.userlist.iter() {
                m.insert(user.email.clone(), user.userid.clone());
            }
            Ok(m)
        }
    }

    async fn refresh_access_token(&mut self) -> Result<()> {
        if let Some(expires_time) = self.expires_time {
            if Local::now().lt(&expires_time) && self.access_token.is_some() {
                return Ok(());
            }
        }

        let api = format!("{}/cgi-bin/gettoken", BASE_API);
        let res = self
            .client
            .get(&api)
            .query(&[("corpid", self.corp_id), ("corpsecret", self.app_secret)])
            .send()
            .await?
            .json::<AccessToken>()
            .await?;
        if res.errcode != 0 {
            Err(anyhow!("Get access token failed: {}", res.errmsg))
        } else {
            self.access_token = Some(res.access_token);
            let t = Local::now().timestamp() + res.expires_in / 2;
            self.expires_time = Some(Local.timestamp(t, 0));
            Ok(())
        }
    }
}