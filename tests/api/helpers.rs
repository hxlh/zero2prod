use once_cell::sync::Lazy;
use sqlx::{Pool, Postgres};
use wiremock::MockServer;
use zero2prod::{configuration::get_config, startup, telemetry::config_logger};

static INIT_LOGGER: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        config_logger("test".into(), "debug".into(), std::io::stdout);
    } else {
        config_logger("test".into(), "debug".into(), std::io::sink);
    };
});

pub struct TestApp {
    pub address: String,
    pub db_conn_pool: Pool<Postgres>,
    pub email_server: MockServer,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("http://{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&INIT_LOGGER);

    // 启动一个模拟服务器来代替 email服务商 的 API
    let email_server = MockServer::start().await;
    
    // configure database
    let config = {
        let mut c = get_config().expect("Failed to load configuration");
        c.db.dbname = format!("test_{}",uuid::Uuid::new_v4().to_string());
        c.app.port = 0;
        c.email.base_url=email_server.uri();
        c
    };

    // init database


    let server = startup::Application::build(config)
        .await
        .expect("Failed to build server");
    let config = server.settings().clone();

    let address = format!(
        "{}:{}",
        server.settings().app.host,
        server.settings().app.port
    );
    tokio::spawn(server.run_until_stopped());

    TestApp {
        address: address,
        db_conn_pool: startup::get_conn_pool(&config.db),
        email_server: email_server,
    }
}
