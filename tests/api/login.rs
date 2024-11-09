use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    // 准备
    let app = spawn_app().await;
    // 行动
    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });
    // Act - Part 1 - 尝试登录
    let response = app.post_login(&login_body).await;
    // 断言
    assert_eq!(response.status().as_u16(), 303);
    assert_is_redirect_to(&response, "/login");

    dbg!(&response);

    let flash_cookie = response.cookies().find(|c| c.name() == "_flash").unwrap();
    assert_eq!(flash_cookie.value(), "Authentication failed");

    // Act - Part 2 - 跟随重定向
    let html = app.get_login_html().await;
    assert!(html.contains(r#"<p><i>Authentication failed</i></p>"#));

    // Act - Part 3 - 重新加载登录页面
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains(r#"<p><i>Authentication failed</i></p>"#));
}
