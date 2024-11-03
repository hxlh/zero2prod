
use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
};
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{Pool, Postgres, Row, Transaction};

#[derive(serde::Deserialize)]
pub struct SubscriptionsData {
    name: String,
    email: String,
}

impl TryInto<NewSubscriber> for SubscriptionsData {
    type Error = String;
    fn try_into(self) -> Result<NewSubscriber, Self::Error> {
        let name = SubscriberName::parse(self.name)?;
        let email = SubscriberEmail::parse(self.email)?;
        Ok(NewSubscriber { name, email })
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool,email_client),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    ),
)]
pub async fn subscriptions(
    form: web::Form<SubscriptionsData>,
    pool: web::Data<Pool<Postgres>>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<String>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form
        .0
        .try_into()
        .map_err(|e| SubscribeError::ValidationError(e))?;

    let mut tx = pool
        .begin()
        .await
        .context("开启事务失败")?;

    // 保存订阅者信息
    let id = save_subscriber(&mut tx, &new_subscriber)
        .await
        .context("保存订阅者信息失败")?;
    // 生成token并保存,同时生成订阅确认链接并发送给用户
    let subscription_token = generate_subscription_token();
    save_subscription_token(&mut tx, id, &subscription_token)
        .await
        .context("保存订阅token失败")?;

    tx.commit()
        .await
        .context("提交事务失败")?;

    send_confirmation_email(
        email_client.as_ref(),
        new_subscriber,
        &base_url,
        &subscription_token,
    )
    .await
    .context("发送确认邮件失败")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "send confirmation email", skip(email_client, new_subscriber))]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    confirm_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?confirm_token={}",
        base_url, confirm_token
    );
    email_client
        .send(
            &new_subscriber.email,
            "Welcome to our newsletter!",
            &format!(
                r#"
            Welcome to our newsletter!
            访问 {} 确认您的订阅。
            "#,
                confirmation_link
            ),
            &format!(
                r#"
            Welcome to our newsletter!
            点击 <a href="{}">此处</a> 确认您的订阅。
            "#,
                confirmation_link
            ),
        )
        .await
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database.",
    skip(subscriber, tx)
)]
async fn save_subscriber(
    tx: &mut Transaction<'_, Postgres>,
    subscriber: &NewSubscriber,
) -> Result<i64, sqlx::Error> {
    let subscriber_email = subscriber.email.as_ref();
    let subscriber_name = subscriber.name.as_ref();

    let id:(i64,) = sqlx::query_as(
        r#"
        INSERT INTO subscriptions (email, name, subscribed_at,status)
        Values ($1,$2,$3,'pending_confirmation')
        RETURNING id
        "#,
    )
    .bind(subscriber_email)
    .bind(subscriber_name)
    .bind(Utc::now())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(sqlx::Error::RowNotFound)
    .map_err(|e| {
        e
    })?;

    Ok(id.0)
}

#[tracing::instrument(
    name ="Saving subscription token in the database."
    skip(tx, id, token)
)]
async fn save_subscription_token(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    token: &str,
) -> Result<(), SaveTokenError> {
    sqlx::query(
        r#"
        insert into subscription_tokens (subscriber_id, subscription_token)
        values ($1, $2)
        "#,
    )
    .bind(id)
    .bind(token)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        SaveTokenError(e)
    })?;

    Ok(())
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

pub struct SaveTokenError(sqlx::Error);

impl std::fmt::Display for SaveTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to save subscription token in the database")
    }
}

impl std::fmt::Debug for SaveTokenError {
    // fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //     write!(
    //         f,
    //         "{}\nCaused by: \n\t{}",
    //         self,
    //         self.0
    //     )
    // }
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for SaveTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n\t", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}
impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl ResponseError for SubscribeError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscribeError::ValidationError(_) => reqwest::StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
