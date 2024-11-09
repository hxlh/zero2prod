use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use once_cell::sync::Lazy;
use rand::thread_rng;
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
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&INIT_LOGGER);

    // 启动一个模拟服务器来代替 email服务商 的 API
    let email_server = MockServer::start().await;

    // configure database
    let config = {
        let mut c = get_config().expect("Failed to load configuration");
        c.db.dbname = format!("test_{}", uuid::Uuid::new_v4());
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

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let app = TestApp {
        address,
        port: config.app.port,
        db_conn_pool: startup::get_conn_pool(&config.db),
        email_server,
        test_user: TestUser::generate(),
        api_client: client,
    };

    app.test_user.store(&app.db_conn_pool).await;
    app
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store(&self, pool: &Pool<Postgres>) {
        //  我们在这里不关心确切的 Argon2 参数，因为它是用于测试目的的！
        let salt = SaltString::generate(&mut thread_rng());

        let pwd_hash = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

        sqlx::query(
            "INSERT INTO users (user_id, username, password_hash)
            VALUES ($1, $2, $3)",
        )
        .bind(self.user_id)
        .bind(&self.username)
        .bind(&pwd_hash)
        .execute(pool)
        .await
        .expect("Failed to store test user.");
    }
}

impl TestApp {
    pub async fn get_admin_dashboard(&self) -> String {
        self.api_client
            .get(format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }

    // 我们的测试将只关注 HTML 页面，因此
    // 我们不暴露底层的 reqwest::Response
    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(format!("{}/login", &self.address))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/login", &self.address))
            // 这个 `reqwest` 方法确保请求体被 URL 编码
            // 并且 `Content-Type` 头部被相应地设置。
            .form(body)
            .send()
            .await
            .expect("执行请求失败。")
    }

    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        self.api_client
            .post(format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        self.api_client
            .post(format!("{}/newsletters", &self.address))
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
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

        let html_link = get_links(body["HtmlBody"].as_str().unwrap());
        let text_link = get_links(body["TextBody"].as_str().unwrap());

        ConfirmationLinks {
            html_link,
            text_link,
        }
    }
}

pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
