use actix_web::{dev::Server, web, App, HttpServer};
use std::net::TcpListener;

use crate::routes;

pub fn run(
    listener: TcpListener,
    pool: sqlx::Pool<sqlx::sqlite::Sqlite>,
) -> Result<Server, std::io::Error> {
    // 用智能指针包装连接
    let conn_pool = web::Data::new(pool);

    let srv = HttpServer::new(move || {
        App::new()
            .wrap(tracing_actix_web::TracingLogger::default())
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscriptions))
            .app_data(conn_pool.clone())
    })
    .listen(listener)?
    .run();

    Ok(srv)
}
