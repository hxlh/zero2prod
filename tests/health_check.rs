use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqlitePool};
use std::{net::TcpListener, str::FromStr};
use zero2prod::{
    configuration::{get_config, Settings},
    telemetry::config_logger,
};

static INIT_LOGGER: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        config_logger("test".into(), "debug".into(), std::io::stdout);
    } else {
        config_logger("test".into(), "debug".into(), std::io::sink);
    };
});

#[tokio::test]
async fn health_check_works() {
    // Arrange
    let address = spawn_app().await;
    // 我们需要引入 `reqwest` 来对应用程序执行 HTTP 请求。
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(format!("http://{}/health_check", address))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let config = get_config().expect("Failed to load configuration");
    let conn_str = config.db.connection_string();
    let app_address = spawn_app().await;

    let client = reqwest::Client::new();
    // Act
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("http://{}/subscriptions", &app_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    // Assert
    assert_eq!(200, response.status().as_u16());

    let mut conn = sqlx::sqlite::SqliteConnection::connect(conn_str.expose_secret())
        .await
        .expect("Failed to connect to database");
    let subscriber =
        sqlx::query!("SELECT * FROM subscriptions WHERE email='ursula_le_guin@gmail.com'")
            .fetch_one(&mut conn)
            .await
            .expect("Failed to fetch saved subscription.");

    assert_eq!(subscriber.name, "le guin");
    assert_eq!(subscriber.email, "ursula_le_guin@gmail.com");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    // Arrange
    let app_address = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        // Act
        let response = client
            .post(&format!("http://{}/subscriptions", &app_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            // 在测试失败时提供自定义的错误消息
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

async fn spawn_app() -> String {
    Lazy::force(&INIT_LOGGER);

    let listener = TcpListener::bind("0.0.0.0:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();

    // configure database
    let mut config = get_config().expect("Failed to load configuration");
    config.db.dbname = uuid::Uuid::new_v4().to_string();
    let pool = config_random_memory_database(&config).await;

    let server = zero2prod::startup::run(listener, pool).expect("Failed to bind address");
    tokio::spawn(server);

    // return address
    format!("0.0.0.0:{}", port)
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
