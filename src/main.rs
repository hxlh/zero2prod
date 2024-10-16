use zero2prod::{
    configuration::{self},
    startup, telemetry,
};

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    telemetry::config_logger("zero2prod".into(), "info".into(), std::io::stdout);

    let settings = configuration::get_config().expect("Failed to load configuration");

    let server = startup::Application::build(settings).await?;
    server.run_until_stopped().await
}
