use actix_web::{get, web, App, HttpServer, Responder, middleware};
use shiplift::Docker;

#[get("/start_container/{container_id}")]
async fn start_container(container_id: web::Path<String>) -> impl Responder {
    let docker = Docker::new();
    let container_id = container_id.to_string();
    if container_id.len().lt(&12usize) {
        return String::from("Container ID is too short");
    }
    match docker.containers().get(container_id.to_string()).start().await {
        Ok(_) => String::from("Container started"),
        Err(e) => e.to_string(),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(start_container)
    })
        .bind(("0.0.0.0", 17456))?
        .run()
        .await
}