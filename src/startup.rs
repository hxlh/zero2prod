use actix_web::{dev::Server, web, App, HttpServer};
use sqlx::{Connection, PgConnection, Pool, Postgres};
use std::{net::TcpListener, time::Duration};

use crate::{configuration::{DatabaseSettings, Settings}, email_client::EmailClient, routes};

pub struct Application {
    settings: Settings,
    server: Server,
}

impl Application {
    pub async fn build(mut settings: Settings) -> Result<Self, std::io::Error> {
        let pool = config_database(&settings.db).await;

        let email_client = EmailClient::new(
            settings.email.base_url.clone(),
            settings
                .email
                .sender()
                .expect("Failed to parse sender email address"),
            Duration::from_secs(10),
        );
        let address = format!("{}:{}", settings.app.host, settings.app.port);

        let listener = TcpListener::bind(&address)?;
        let port=listener.local_addr().unwrap().port();
        settings.app.port=port;

        Ok(Self {
            settings: settings,
            server: run(listener, pool, email_client)?,
        })
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}


pub fn get_conn_pool(settings: &DatabaseSettings) -> Pool<Postgres> {
    Pool::connect_lazy_with(settings.with_db())
}

async fn config_database(settings: &DatabaseSettings) -> Pool<Postgres> {
    let mut conn=PgConnection::connect_with(&settings.without_db())
    .await.expect("Failed to connect to database");

    // 创建数据库
    sqlx::query(&format!(r#"CREATE DATABASE "{}";"#, settings.dbname))
    .execute(&mut conn)
    .await
    .expect("Failed to create database");

    let pool=Pool::connect_with(settings.with_db())
    .await.expect("Failed to connect to database");
    // 迁移数据库
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to migrate database");

    pool
}

pub fn run(
    listener: TcpListener,
    db_conn_pool: Pool<Postgres>,
    email_client: EmailClient,
) -> Result<Server, std::io::Error> {
    // 用智能指针包装连接
    let db_conn_pool = web::Data::new(db_conn_pool);
    let email_client = web::Data::new(email_client);

    let srv = HttpServer::new(move || {
        App::new()
            .wrap(tracing_actix_web::TracingLogger::default())
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscriptions))
            .app_data(db_conn_pool.clone())
            .app_data(email_client.clone())
    })
    .listen(listener)?
    .run();

    Ok(srv)
}
