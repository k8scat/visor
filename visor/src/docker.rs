use chrono::prelude::{DateTime, Utc};
use std::ops::Sub;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use log::{info, warn};
use regex::Regex;
use shiplift::rep::Container;
use shiplift::{ContainerFilter, Docker};

use crate::config::Config;
use crate::instance::{get_instance, Instance};
use crate::notify::{message_tpl, Notifier};
use crate::psutil::{get_cpu_usage, get_disk_usage, get_mem_usage};

// https://docs.docker.com/engine/reference/commandline/ps/#filtering
pub async fn list_exited_containers(docker: &Docker) -> Result<Vec<Container>> {
    let opts = shiplift::ContainerListOptions::builder()
        .filter(vec![ContainerFilter::Status(String::from("exited"))])
        .build();
    let containers = docker.containers().list(&opts).await?;
    Ok(containers)
}

pub async fn list_running_containers(docker: &Docker) -> Result<Vec<Container>> {
    let opts = shiplift::ContainerListOptions::builder()
        .filter(vec![ContainerFilter::Status(String::from("running"))])
        .build();
    let containers = docker.containers().list(&opts).await?;
    Ok(containers)
}

pub async fn stop_container(docker: &Docker, id: &str) -> Result<()> {
    Ok(docker.containers().get(id).stop(None).await?)
}

pub async fn remove_container(docker: &Docker, id: &str) -> Result<()> {
    Ok(docker
        .containers()
        .get(id)
        .remove(Default::default())
        .await?)
}

pub async fn clean_images(docker: &Docker, lifecycle: u64) -> Result<()> {
    let images = docker.images().list(&Default::default()).await?;
    let t: DateTime<Utc> = SystemTime::now()
        .sub(Duration::from_secs(lifecycle * 86400))
        .into();
    for image in images.iter() {
        if image.created.gt(&t) {
            info!("Ignore image: {}", image.id);
            continue;
        }

        if let Err(e) = docker.images().get(&image.id).delete().await {
            warn!("Delete image {} failed: {}", image.id, e);
        } else {
            info!("Deleted image {}", image.id);
        }
    }
    Ok(())
}

pub async fn clean_volumes(docker: &Docker) -> Result<()> {
    let volumes = docker.volumes().list().await?;
    for volume in volumes.iter() {
        if let Err(e) = docker.volumes().get(&volume.name).delete().await {
            warn!("Delete volume {} failed: {}", volume.name, e);
        } else {
            info!("Deleted volume {}", volume.name);
        }
    }
    Ok(())
}

pub fn parse_status_time(mut s: String) -> Vec<String> {
    let re = Regex::new(r"(Exited |Up )(\([0-9]+\) )?").unwrap();
    s = re.replace_all(&s, "").to_string();
    let items = s.split(" ").collect::<Vec<&str>>();
    vec![items[0].to_string(), items[1].to_string()]
}

pub fn status_into_running_time(s: String) -> Result<Duration> {
    let items = parse_status_time(s);
    let num = items[0].parse::<u64>().unwrap_or_default();
    let unit = items[1].clone();
    match unit.as_str() {
        "seconds" => Ok(Duration::from_secs(num)),
        "minutes" => Ok(Duration::from_secs(num * 60)),
        "hours" => Ok(Duration::from_secs(num * 60 * 60)),
        "days" => Ok(Duration::from_secs(num * 60 * 60 * 24)),
        "weeks" => Ok(Duration::from_secs(num * 60 * 60 * 24 * 7)),
        "months" => Ok(Duration::from_secs(num * 60 * 60 * 24 * 7 * 30)),
        "years" => Ok(Duration::from_secs(num * 60 * 60 * 24 * 7 * 30 * 365)),
        _ => Err(anyhow::anyhow!("Unknown unit: {}", unit)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_status_time() {
        let s = "Up 2 weeks";
        let items = super::parse_status_time(s.to_string());
        assert_eq!(items[0], "2");
        assert_eq!(items[1], "weeks");

        let s = "Exited (137) 9 hours ago";
        let items = super::parse_status_time(s.to_string());
        assert_eq!(items[0], "9");
        assert_eq!(items[1], "hours");
    }
}

pub async fn stop_containers<T>(docker: &Docker, cfg: &Config, notifier: &T) -> Result<()>
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

pub async fn clean_exited_containers(docker: &Docker, lifecycle: u64) -> Result<()> {
    let containers = list_exited_containers(docker).await?;
    if containers.is_empty() {
        info!("No exited containers found");
        return Ok(());
    }

    let d = Duration::from_secs(86400 * lifecycle);
    for container in containers.iter() {
        let t = status_into_running_time(container.status.clone()).unwrap_or_default();
        if t.lt(&d) {
            info!("Ignore container {}", &container.id);
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
