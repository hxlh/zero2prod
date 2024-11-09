use actix_web::{
    dev::Server,
    web::{self},
    App, HttpServer,
};
use secrecy::Secret;
use sqlx::{Connection, PgConnection, Pool, Postgres};
use std::{net::TcpListener, time::Duration};

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes,
};

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
        let port = listener.local_addr().unwrap().port();
        settings.app.port = port;

        let server = run(
            listener,
            pool,
            email_client,
            settings.app.base_url.clone(),
            settings.app.hmac_secret.clone(),
        )?;
        Ok(Self { settings, server })
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
    let mut conn = PgConnection::connect_with(&settings.without_db())
        .await
        .expect("Failed to connect to database");

    let database_exists = sqlx::query("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(&settings.dbname)
        .fetch_one(&mut conn)
        .await
        .is_ok();

    // 如果数据库不存在，则创建
    if !database_exists {
        // 创建数据库
        sqlx::query(&format!(r#"CREATE DATABASE "{}";"#, settings.dbname))
            .execute(&mut conn)
            .await
            .expect("Failed to create database");
    }

    let pool = Pool::connect_with(settings.with_db())
        .await
        .expect("Failed to connect to database");
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
    base_url: String,
    hmac_secret: Secret<String>,
) -> Result<Server, std::io::Error> {
    // 用智能指针包装连接
    let db_conn_pool = web::Data::new(db_conn_pool);
    let email_client = web::Data::new(email_client);

    let srv = HttpServer::new(move || {
        App::new()
            .wrap(tracing_actix_web::TracingLogger::default())
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscriptions))
            .route("/subscriptions/confirm", web::get().to(routes::confirm))
            .route("/newsletters", web::post().to(routes::publish_newsletter))
            .route("/", web::get().to(routes::home))
            .route("/login", web::get().to(routes::login_from))
            .route("/login", web::post().to(routes::login))
            .app_data(db_conn_pool.clone())
            .app_data(email_client.clone())
            .app_data(web::Data::new(base_url.clone()))
            .app_data(web::Data::new(hmac_secret.clone()))
    })
    .listen(listener)?
    .run();

    Ok(srv)
}
