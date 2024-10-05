use std::net::TcpListener;

use zero2prod::{configuration, startup};

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = configuration::get_config().expect("Failed to load configuration");
    let address = format!("{}:{}", config.app_host, config.app_port);
    let listener = TcpListener::bind(address).expect("Failed to bind to address");
    startup::run(listener)?.await
}

// fn main() {
 
// }
