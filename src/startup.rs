use actix_web::{dev::Server, web, App, HttpServer};
use std::net::TcpListener;

use crate::routes;

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let srv = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscriptions))
    })
    .listen(listener)?
    .run();

    Ok(srv)
}