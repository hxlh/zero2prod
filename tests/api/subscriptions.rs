use crate::helpers::spawn_app;

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app().await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = app.post_subscriptions(body.into()).await;
    // Assert
    assert_eq!(200, response.status().as_u16());

    let conn = &app.db_conn_pool;
    let subscriber =
        sqlx::query!("SELECT * FROM subscriptions WHERE email='ursula_le_guin@gmail.com'")
            .fetch_one(conn)
            .await
            .expect("Failed to fetch saved subscription.");

    assert_eq!(subscriber.name, "le guin");
    assert_eq!(subscriber.email, "ursula_le_guin@gmail.com");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        // Act
        let response = app.post_subscriptions(invalid_body.into()).await;
        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            // 在测试失败时提供自定义的错误消息
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn subscribe_returns_a_200_when_fields_are_present_but_empty() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name", 400),
        ("name=Ursula&email=", "empty email", 400),
        (
            "name=Ursula&email=definitely-not-an-email",
            "invalid email",
            400,
        ),
    ];
    for (body, description, code) in test_cases {
        // Act
        let response = app.post_subscriptions(body.into()).await;
        // Assert
        assert_eq!(
            code,
            response.status().as_u16(),
            "The API should not return {} when the payload was {}.",
            response.status().as_u16(),
            description
        );
    }
}
