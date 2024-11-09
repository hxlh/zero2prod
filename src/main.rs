use zero2prod::{
    configuration::{self},
    startup, telemetry,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::config_logger("zero2prod".into(), "info".into(), std::io::stdout);

    let settings = configuration::get_config().expect("Failed to load configuration");

    let server = startup::Application::build(settings).await?;
    Ok(server.run_until_stopped().await?)
}
