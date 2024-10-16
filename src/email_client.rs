use std::time::Duration;

use reqwest::Client;

use crate::domain::SubscriberEmail;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail, timeout: Duration) -> Self {
        Self {
            http_client: Client::builder().timeout(timeout).build().unwrap(),
            base_url,
            sender,
        }
    }

    #[allow(unused)]
    pub async fn send(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        content: &str,
        html_content: &str,
    ) -> Result<(), reqwest::Error> {
        let _ = self.sender;
        let url = format!("{}/email", self.base_url);
        let _ = self
            .http_client
            .post(&url)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use claim::{assert_err, assert_ok};
    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake,
    };
    use wiremock::{
        matchers::{any, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::{domain::SubscriberEmail, email_client::EmailClient};

    /// 生成随机电子邮件主题
    fn subject() -> String {
        Sentence(1..2).fake()
    }

    /// 生成随机电子邮件内容
    fn content() -> String {
        Paragraph(1..10).fake()
    }

    /// 生成随机订阅者电子邮件
    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    /// 获取 EmailClient 的测试实例。
    fn email_client(base_url: String) -> EmailClient {
        EmailClient::new(base_url, email(), Duration::from_secs(1))
    }

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        // Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());
        Mock::given(path("/email"))
            .and(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();
        // Act
        let res = email_client
            .send(subscriber_email, &subject, &content, &content)
            .await;
        // Assert
        assert_ok!(res);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        // Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());
        Mock::given(path("/email"))
            .and(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();
        // Act
        let res = email_client
            .send(subscriber_email, &subject, &content, &content)
            .await;
        // Assert
        assert_err!(res);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        // Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

        let response = ResponseTemplate::new(200)
            // 3 分钟！
            .set_delay(std::time::Duration::from_secs(180));

        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let outcome = email_client
            .send(subscriber_email, &subject, &content, &content)
            .await;

        // Assert
        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        // Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(path("/email"))
            .and(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let res = email_client
            .send(email(), &subject(), &content(), &content())
            .await;
        // Assert
        assert_ok!(res);
    }
}
