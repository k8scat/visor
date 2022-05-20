use actix_web::{get, web, HttpRequest, HttpResponse};
use bollard::errors::Error;
use bollard::Docker;

#[get("/start_container/{container_id}")]
async fn start_container(req: HttpRequest, container_id: web::Path<String>) -> HttpResponse {
    req.headers().iter().for_each(|(k, v)| {
        println!("{}={:?}", k, v);
    });

    let docker = Docker::connect_with_socket_defaults().unwrap();
    let container_id = container_id.as_str();
    if container_id.len().lt(&12usize) {
        return html(String::from("无效的容器ID"));
    }

    match docker.start_container::<String>(container_id, None).await {
        Ok(_) => html(String::from("容器启动成功")),
        Err(e) => match e {
            Error::DockerResponseServerError {
                status_code,
                message,
            } => {
                if status_code.eq(&304) {
                    return html(String::from("容器已启动"));
                } else {
                    return html(format!("启动容器失败：{}", message));
                }
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
