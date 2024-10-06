use secrecy::Secret;

#[derive(serde::Deserialize)]
pub struct Settings {
    pub db: DatabaseSettings,
    pub app: ApplicationSettings,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub dbname: String,
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!("sqlite:{}.db", self.dbname))
    }
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    let s = config::Config::builder()
        .add_source(config::File::with_name("config"))
        .build()?;

    s.try_deserialize()
}
