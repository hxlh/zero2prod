use secrecy::{ExposeSecret, Secret};
use sqlx::postgres::PgConnectOptions;

use crate::domain::SubscriberEmail;

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub app: ApplicationSettings,
    pub db: DatabaseSettings,
    pub email: EmailClientSettings,
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub user: String,
    pub password: Secret<String>,
    pub dbname: String,
    pub host: String,
    pub port: u16,
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!("postgres://{}:{}@{}:{}/{}",self.user,self.password.expose_secret(),self.host,self.port,self.dbname))
    }

    pub fn without_db(&self) -> PgConnectOptions{
        PgConnectOptions::new()
        .username(&self.user)
        .password(self.password.expose_secret())
        .host(&self.host)
        .port(self.port)
    }

    pub fn with_db(&self) -> PgConnectOptions{
        self.without_db().database(&self.dbname)
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    let s = config::Config::builder()
        .add_source(config::File::with_name("config"))
        .build()?;

    s.try_deserialize()
}
