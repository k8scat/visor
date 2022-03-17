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
