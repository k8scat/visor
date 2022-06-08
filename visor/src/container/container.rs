use crate::config::Config;
use crate::instance::{get_instance, Instance};
use crate::notify::{message_tpl, Notifier};
use crate::psutil::{get_cpu_usage, get_disk_usage, get_mem_usage};
use crate::wechat::wechat::Wechat;
use anyhow::{anyhow, Result};
use bollard::container::ListContainersOptions;
use bollard::errors::Error;
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

pub async fn clean_images(docker: &Docker, cfg: &Config) -> Result<()> {
    let opts = ListImagesOptions::<String> {
        all: true,
        ..Default::default()
    };
    let images = docker.list_images(Some(opts)).await?;

    let t = (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
        - cfg.lifecycle.image_created * 86400) as i64;
    for image in images.iter() {
        // Check image tag
        let mut tag_found = false;
        for tag in image.repo_tags.iter() {
            if let Some(images_map) = &cfg.whitelist.images_map {
                if images_map.contains(tag) {
                    info!("Ignored: image {} is in the whitelist", tag);
                    tag_found = true;
                    break;
                }
            }
        }
        if tag_found {
            continue;
        }

        // Check image id
        if let Some(images_map) = &cfg.whitelist.images_map {
            if images_map.contains(&image.id) {
                info!("Ignored: image {} is in the whitelist", image.id);
                continue;
            }
        }
        
        if image.created.gt(&t) {
            info!("Ignored: image {} was created {}", image.id, image.created);
            continue;
        }
        if let Err(e) = docker.remove_image(&image.id, None, None).await {
            if let Error::DockerResponseServerError {
                status_code,
                message,
            } = e
            {
                if status_code == 500 {
                    warn!("Delete image {} failed: {}", image.id, message);
                }
            }
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
            if let Error::DockerResponseServerError {
                status_code,
                message,
            } = e
            {
                if status_code == 500 {
                    warn!("Delete volume {} failed: {}", volume.name, message);
                }
            }
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

pub fn status_into_duration(s: &str) -> Result<Duration> {
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
                // Check container id
                if let Some(id) = &c.id.clone() {
                    if let Some(containers_map) = &cfg.whitelist.containers_map {
                        if containers_map.contains(id) {
                            info!("Ignored: container {} is in the whitelist", id);
                            return false;
                        }
                    }
                }

                // Check container name
                if let Some(names) = c.names.clone() {
                    for name in names.iter() {
                        if let Some(containers_map) = &cfg.whitelist.containers_map {
                            if containers_map.contains(name) {
                                info!("Ignored: container {} is in the whitelist", name);
                                return false;
                            }
                        }
                    }
                }
                true
            })
            .collect();
        if containers.is_empty() {
            info!("No running containers found");
            return Ok(());
        }

        containers.sort_by(|a, b| {
            let a_time =
                status_into_duration(&a.status.clone().unwrap_or_default()).unwrap_or_default();
            let b_time =
                status_into_duration(&b.status.clone().unwrap_or_default()).unwrap_or_default();
            b_time.cmp(&a_time)
        });

        let container = &containers[0];
        let container_id = &container.id.clone().unwrap_or_default();
        let instance = get_instance(&container).unwrap_or_else(|e| {
            warn!("Get instance owner failed: {}", e);
            Instance::default()
        });
        info!("Owner email: {}", instance.owner);

        if let Err(e) = docker.stop_container(container_id, None).await {
            if let Error::DockerResponseServerError {
                status_code,
                message,
            } = e
            {
                if status_code == 500 {
                    return Err(anyhow!("{}", message));
                }
            }
        }
        info!("Stopped container: {}", container_id);

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

pub struct ComplexContainer {
    pub container: ContainerSummary,
    pub exist_duration: Duration,
}

pub async fn map_existed_containers(docker: &Docker) -> Result<Option<HashMap<String, ComplexContainer>>> {
    let containers = list_exited_containers(docker).await?;
    if containers.is_empty() {
        return Ok(None);
    }

    let mut containers_map: HashMap<String, ComplexContainer> = HashMap::new();
    for container in containers {
        if let Some(container_id) = &container.id {
            if let Some(status) = &container.status {
                let exist_duration = status_into_duration(status);
                match exist_duration {
                    Ok(exist_time) => {
                        containers_map.insert(container_id.clone(), ComplexContainer {
                            container,
                            exist_duration: exist_time,
                        });
                    }
                    Err(e) => {
                        warn!("Status into duration failed: {}, err: {}", status, e);
                    }
                }
            } else {
                warn!("Container status not found: {:?}", container);
            }
        } else {
            warn!("Container id not found: {:?}", container)
        }
    }
    return Ok(Some(containers_map));
}

pub async fn clean_exited_containers(docker: &Docker, lifecycle: u64, existed_containers_map: &Option<HashMap<String, ComplexContainer>>) -> Result<()> {
    if let Some(existed_containers_map) = existed_containers_map {
        let d = Duration::from_secs(86400 * lifecycle);
        for (container_id, container) in existed_containers_map {
            if container.exist_duration.lt(&d) {
                info!("Ignored: container {} exited {} seconds", container_id, d.as_secs());
                continue;
            }

            if let Err(e) = docker.remove_container(&container_id, None).await {
                if let Error::DockerResponseServerError {
                    status_code,
                    message,
                } = e
                {
                    if status_code == 500 {
                        warn!("Remove container {} failed: {}", container_id, message);
                    }
                }
            } else {
                info!("Removed container {}", container_id);
            }
        }
    } else {
        info!("No exited containers found");
        return Ok(());
    }
    Ok(())
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

    use bollard::Docker;

    #[tokio::test]
    async fn test_list_images() {
        let docker = Docker::connect_with_socket_defaults().unwrap();
        let images = docker.list_images::<String>(None).await.unwrap();
        println!("{:?}", images);
    }
}
