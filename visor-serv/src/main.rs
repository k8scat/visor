mod services;

use crate::services::start_container;
use actix_web::{middleware, App, HttpServer};

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
