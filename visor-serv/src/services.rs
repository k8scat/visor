use actix_web::{get, web, HttpRequest, HttpResponse};
use shiplift::Docker;

#[get("/start_container/{container_id}")]
async fn start_container(req: HttpRequest, container_id: web::Path<String>) -> HttpResponse {
    req.headers().iter().for_each(|(k, v)| {
        println!("{}={:?}", k, v);
    });

    let docker = Docker::new();
    let container_id = container_id.to_string();
    if container_id.len().ne(&64usize) {
        return html(String::from("无效的容器ID"));
    }

    let container = docker.containers().get(container_id.to_string());

    match container.start().await {
        Ok(_) => html(String::from("容器已启动")),
        Err(e) => {
            let e = e.to_string();
            if e.contains("304 Not Modified") {
                html(format!(r#"容器已启动 <p style="display: none;">{}</p>"#, e))
            } else {
                html(format!("容器启动失败: {}", e))
            }
        }
    }
}

fn html(content: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(content)
}
