use std::time::Duration;

use anyhow::Result;
use log::warn;
use regex::Regex;
use shiplift::{ContainerFilter, Docker};
use shiplift::rep::Container;

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
    Ok(docker.containers().get(id).remove(Default::default()).await?)
}

pub async fn image_prune(docker: &Docker) -> Result<()> {
    let images = docker.images().list(&Default::default()).await?;
    for image in images.iter() {
        if let Err(e) = docker.images().get(&image.id).delete().await {
            warn!("Failed to delete image {}: {}", image.id, e);
        }
    }
    Ok(())
}

pub async fn volume_prune(docker: &Docker) -> Result<()> {
    let volumes = docker.volumes().list().await?;
    for volume in volumes.iter() {
        if let Err(e) = docker.volumes().get(&volume.name).delete().await {
            warn!("Failed to delete volume {}: {}", volume.name, e);
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

pub fn status_into_time(s: String) -> Result<Duration> {
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
