use crate::config::Config;
use crate::instance::{get_instance, Instance};
use crate::notify::{message_tpl, Notifier};
use crate::psutil::{get_cpu_usage, get_disk_usage, get_mem_usage};
use crate::wechat::wechat::Wechat;
use anyhow::{anyhow, Result};
use bollard::container::ListContainersOptions;
use bollard::image::ListImagesOptions;
use bollard::models::ContainerSummary;
use bollard::Docker;
use log::{info, warn};
use regex::Regex;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// https://docs.docker.com/engine/reference/commandline/ps/#filtering
pub async fn list_containers_by_status(
    docker: &Docker,
    status: Vec<&str>,
) -> Result<Vec<ContainerSummary>> {
    let mut filters = HashMap::new();
    filters.insert("status", status);

    let opts = ListContainersOptions {
        all: true,
        filters,
        ..Default::default()
    };
    let containers = docker.list_containers(Some(opts)).await?;
    Ok(containers)
}

pub async fn list_exited_containers(docker: &Docker) -> Result<Vec<ContainerSummary>> {
    Ok(list_containers_by_status(docker, vec!["exited"]).await?)
}

pub async fn list_running_containers(docker: &Docker) -> Result<Vec<ContainerSummary>> {
    Ok(list_containers_by_status(docker, vec!["running"]).await?)
}

pub async fn stop_container(docker: &Docker, container_name: &str) -> Result<()> {
    Ok(docker.stop_container(container_name, None).await?)
}

pub async fn remove_container(docker: &Docker, container_name: &str) -> Result<()> {
    Ok(docker.remove_container(container_name, None).await?)
}

pub async fn clean_images(docker: &Docker, cfg: &Config) -> Result<()> {
    let opts = ListImagesOptions::<String> {
        all: true,
        ..Default::default()
    };
    let images = docker.list_images(Some(opts)).await?;

    let t = (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
        - cfg.lifecycle.image_created * 86400) as i64;
    for image in images.iter() {
        if cfg.whitelist.images_map.contains_key(&image.id) {
            info!("Ignored: image {} is in the whitelist", image.id);
            continue;
        }
        if image.created.gt(&t) {
            info!("Ignored: image {} was created {}", image.id, image.created);
            continue;
        }
        if let Err(e) = docker.remove_image(&image.id, None, None).await {
            warn!("Delete image {} failed: {}", image.id, e);
        } else {
            info!("Deleted image {}", image.id);
        }
    }
    Ok(())
}

pub async fn clean_volumes(docker: &Docker) -> Result<()> {
    let res = docker.list_volumes::<String>(None).await?;
    for volume in res.volumes.iter() {
        if let Err(e) = docker.remove_volume(&volume.name, None).await {
            warn!("Delete volume {} failed: {}", volume.name, e);
        } else {
            info!("Deleted volume {}", volume.name);
        }
    }
    Ok(())
}

pub fn parse_status_time(s: &str) -> Vec<String> {
    let mut s = s.to_string();
    let re = Regex::new(r"(Exited |Up )(\([0-9]+\) )?").unwrap();
    s = re.replace_all(&s, "").to_string();
    let items = s.split(" ").collect::<Vec<&str>>();
    vec![items[0].to_string(), items[1].to_string()]
}

pub fn status_into_running_time(s: &str) -> Result<Duration> {
    if s.is_empty() {
        return Err(anyhow!("Empty status"));
    }

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
        let items = super::parse_status_time(s);
        assert_eq!(items[0], "2");
        assert_eq!(items[1], "weeks");

        let s = "Exited (137) 9 hours ago";
        let items = super::parse_status_time(s);
        assert_eq!(items[0], "9");
        assert_eq!(items[1], "hours");
    }
}

pub async fn stop_containers<'a, T>(
    docker: &Docker,
    cfg: &Config,
    notifier: &T,
    wechat: &mut Wechat<'a>,
) -> Result<()>
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

        let mut containers: Vec<ContainerSummary> = list_running_containers(docker)
            .await?
            .into_iter()
            .filter(|c| {
                if let Some(id) = &c.id.clone() {
                    if cfg.whitelist.containers_map.contains_key(id) {
                        info!("Ignored: container {} is in the whitelist", id);
                        false
                    } else {
                        true
                    }
                } else {
                    true
                }
            })
            .collect();
        if containers.is_empty() {
            info!("No running containers found");
            return Ok(());
        }

        containers.sort_by(|a, b| {
            let a_time =
                status_into_running_time(&a.status.clone().unwrap_or_default()).unwrap_or_default();
            let b_time =
                status_into_running_time(&b.status.clone().unwrap_or_default()).unwrap_or_default();
            b_time.cmp(&a_time)
        });

        let container = &containers[0];
        let container_id = &container.id.clone().unwrap_or_default();
        let instance = get_instance(&container).unwrap_or_else(|e| {
            warn!("Get instance owner failed: {}", e);
            Instance::default()
        });
        info!("Owner email: {}", instance.owner);

        stop_container(docker, container_id).await?;
        info!("Stop container: {}", container_id);

        let mut user_id: Option<&String>;
        user_id = wechat.users.get(&instance.owner);
        if user_id.is_none() {
            wechat
                .map_users_by_department(cfg.wechat.department_id)
                .await?;
            user_id = wechat.users.get(&instance.owner);
        }

        let msg = message_tpl(container, &instance, &cfg);
        notifier.notify(&msg, user_id).await?;
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
        if container.id.is_none() {
            continue;
        }
        let container_id = container.id.clone().unwrap_or_default();

        let t = status_into_running_time(&container.status.clone().unwrap_or_default())
            .unwrap_or_default();
        if t.lt(&d) {
            info!(
                "Ignored: container {} exited {} seconds",
                container_id,
                d.as_secs()
            );
            continue;
        }

        if let Err(e) = remove_container(docker, &container_id).await {
            warn!("Remove container {} failed: {}", container_id, e);
        } else {
            info!("Removed container {}", container_id);
        }
    }
    Ok(())
}
