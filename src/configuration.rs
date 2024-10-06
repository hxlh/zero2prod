use secrecy::Secret;

#[derive(serde::Deserialize)]
pub struct Settings {
    pub db: DatabaseSettings,
    pub app_host: String,
    pub app_port: u16,
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
