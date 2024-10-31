use reqwest::Url;
use wiremock::{matchers::{method, path}, Mock, ResponseTemplate};

use crate::helpers::spawn_app;

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called(){
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .mount(&app.email_server)
    .await;

    app.post_subscriptions(body.into()).await;

    let req=&app.email_server.received_requests().await.unwrap()[0];
    let body: serde_json::Value = req.body_json().unwrap();

    // 提取请求字段中的链接
    let get_link = |s: &str| {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();
        assert_eq!(links.len(), 1);
        links[0].as_str().to_owned()
    };

    let html_link = &get_link(&body["HtmlBody"].as_str().unwrap());
    let mut confirm_link=Url::parse(&html_link).unwrap();

    assert_eq!(confirm_link.host_str().unwrap(),"127.0.0.1");

    confirm_link.set_port(Some(app.port)).unwrap();

    let resp=reqwest::get(confirm_link).await.unwrap();
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

