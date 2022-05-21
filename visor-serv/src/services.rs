use actix_web::{get, web, HttpRequest, HttpResponse};
use bollard::errors::Error;
use bollard::Docker;

#[get("/start_container/{container_id}")]
async fn start_container(req: HttpRequest, container_id: web::Path<String>) -> HttpResponse {
    if let Some(ua) = req.headers().get("user-agent") {
        let s = ua.to_str().unwrap_or_default();
        if !s.to_lowercase().contains("micromessenger") {
            return html(String::from("请通过微信浏览器打开"));
        }
    }

    let docker = Docker::connect_with_socket_defaults().unwrap();
    let container_id = container_id.as_str();
    if container_id.len().lt(&12usize) {
        return html(String::from("无效的容器ID"));
    }

    match docker.start_container::<String>(container_id, None).await {
        Ok(_) => html(String::from("容器已启动")),
        Err(e) => match e {
            Error::DockerResponseServerError {
                status_code,
                message,
            } => {
                return html(format!("启动容器失败：{} {}", status_code, message));
            }
            _ => html(format!("容器启动失败: {}", e)),
        },
    }
}

fn html(content: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(content)
}
