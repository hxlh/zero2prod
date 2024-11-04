use once_cell::sync::Lazy;
use sqlx::{Pool, Postgres};
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::{configuration::get_config, startup, telemetry::config_logger};

static INIT_LOGGER: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        config_logger("test".into(), "debug".into(), std::io::stdout);
    } else {
        config_logger("test".into(), "debug".into(), std::io::sink);
    };
});

pub struct ConfirmationLinks {
    pub html_link: reqwest::Url,
    pub text_link: reqwest::Url,
}

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_conn_pool: Pool<Postgres>,
    pub email_server: MockServer,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        let (username, password) = self.test_user().await;
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &self.address))
            .basic_auth(username, Some(password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn get_confirm_links_from_req(&self, req: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = req.body_json().unwrap();
        let get_links = |text: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(text)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            // 确保我们没有调用随机的网络 API
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html_link = get_links(&body["HtmlBody"].as_str().unwrap());
        let text_link = get_links(&body["TextBody"].as_str().unwrap());

        ConfirmationLinks {
            html_link,
            text_link,
        }
    }

    pub async fn test_user(&self) -> (String, String) {
        let row =
            sqlx::query_as::<_, (String, String)>("SELECT username, password FROM users LIMIT 1")
                .fetch_one(&self.db_conn_pool)
                .await
                .expect("Failed to create test users.");
        (row.0, row.1)
    }
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&INIT_LOGGER);

    // 启动一个模拟服务器来代替 email服务商 的 API
    let email_server = MockServer::start().await;

    // configure database
    let config = {
        let mut c = get_config().expect("Failed to load configuration");
        c.db.dbname = format!("test_{}", uuid::Uuid::new_v4().to_string());
        c.app.port = 0;
        c.email.base_url = email_server.uri();
        c
    };

    // init database

    let server = startup::Application::build(config)
        .await
        .expect("Failed to build server");
    let config = server.settings().clone();

    let address = format!(
        "http://{}:{}",
        server.settings().app.host,
        server.settings().app.port
    );
    tokio::spawn(server.run_until_stopped());

    let app=TestApp {
        address: address,
        port: config.app.port,
        db_conn_pool: startup::get_conn_pool(&config.db),
        email_server: email_server,
    };

    add_test_user(&app.db_conn_pool).await;

    app
}

async fn add_test_user(pool: &Pool<Postgres>) {
    sqlx::query(
        r#"
        INSERT INTO users (user_id, username, password)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .expect("Failed to create test user");
}
