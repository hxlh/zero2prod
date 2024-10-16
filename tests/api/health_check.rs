use crate::helpers::spawn_app;
#[tokio::test]
async fn health_check_works() {
    // Arrange
    let app = spawn_app().await;
    // 我们需要引入 `reqwest` 来对应用程序执行 HTTP 请求。
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(format!("http://{}/health_check", app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
