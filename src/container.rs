use std::time::Duration;
use shiplift::Docker;
use shiplift::rep::Container;
use anyhow::Result;

pub async fn list_containers(docker: &Docker) -> Result<Vec<Container>> {
    let containers = docker.containers().list(&Default::default()).await?;
    Ok(containers)
}

pub async fn stop_container(docker: &Docker, id: &str) -> Result<()> {
    docker.containers().get(id).stop(None).await?;
    Ok(())
}

pub fn parse_status_time(mut s: String) -> Vec<String> {
    s = s.replace("Up ", "");
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
