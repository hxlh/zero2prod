use secrecy::ExposeSecret;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{net::TcpListener, str::FromStr};
use zero2prod::{
    configuration::{self, Settings},
    telemetry,
};

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    telemetry::config_logger("zero2prod".into(), "info".into(), std::io::stdout);

    // configure database
    let mut config = configuration::get_config().expect("Failed to load configuration");
    config.db.dbname = uuid::Uuid::new_v4().to_string();
    let pool = config_random_memory_database(&config).await;

    let listener = TcpListener::bind(format!("{}:{}", config.app_host, config.app_port))
        .expect("Failed to bind random port");
    zero2prod::startup::run(listener, pool)
        .expect("Failed to bind address")
        .await
}

async fn config_random_memory_database(settings: &Settings) -> SqlitePool {
    // 创建数据库
    let conn_str = settings.db.connection_string();
    let options = SqliteConnectOptions::from_str(conn_str.expose_secret())
        .expect("Failed to parse connection string")
        .in_memory(true)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options)
        .await
        .expect("Failed to connect to database");

    // 迁移数据库
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to migrate database");

    pool
}
