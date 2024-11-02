use sqlx::Row;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::spawn_app;

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let req = &app.email_server.received_requests().await.unwrap()[0];

    let confirmation = app.get_confirm_links_from_req(req);

    let resp = reqwest::get(confirmation.html_link).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    let app = spawn_app().await;

    let resp = reqwest::get(format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 400);
}

// 
#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    Mock::given(path("/email"))
       .and(method("POST"))
       .respond_with(ResponseTemplate::new(200))
       .mount(&app.email_server)
       .await;

    app.post_subscriptions(body.into()).await;

    let req = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation = app.get_confirm_links_from_req(req);

    reqwest::get(confirmation.html_link).
    await
    .unwrap()
    .error_for_status()
    .unwrap();

    let row=sqlx::query("SELECT email, name, status FROM subscriptions")
    .fetch_one(&app.db_conn_pool)
    .await
    .expect("Failed to fetch subscription");

    let email:&str=row.try_get("email").unwrap();
    let name:&str=row.try_get("name").unwrap();
    let status:&str=row.try_get("status").unwrap();

    assert_eq!(email, "ursula_le_guin@gmail.com");
    assert_eq!(name, "le guin");
    assert_eq!(status, "confirmed");
}
