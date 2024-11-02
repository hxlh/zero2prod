use std::fmt::write;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
};
use actix_web::{web, HttpResponse, Responder, ResponseError};
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{Pool, Postgres, Row, Transaction};
use tracing_log::log::{self};

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
    let new_subscriber = form.0.try_into()?;

    let mut tx = pool.begin().await
    .map_err(|e| SubscribeError::PoolError(e))?;

    // 保存订阅者信息
    let id = save_subscriber(&mut tx, &new_subscriber).await
    .map_err(|e| SubscribeError::InsertSubscriberError(e))?;
    // 生成token并保存,同时生成订阅确认链接并发送给用户
    let subscription_token = generate_subscription_token();
    save_subscription_token(&mut tx, id, &subscription_token).await?;

    tx.commit().await
    .map_err(|e| SubscribeError::TransactionCommitError(e))?;

    send_confirmation_email(
        email_client.as_ref(),
        new_subscriber,
        &base_url,
        &subscription_token,
    )
    .await?;

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
            new_subscriber.email,
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

    let row = sqlx::query(
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
        tracing::error!(
            "Failed to save new subscriber details in the database: {}",
            e
        );
        e
    })?;

    let id: i64 = row.get(0);

    Ok(id)
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
        tracing::error!("Failed to execute query: {:?}", e);
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

fn error_chain_fmt(
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


pub enum SubscribeError {
    ValidationError(String),
    SaveTokenError(SaveTokenError),
    SendEmailError(reqwest::Error),
    PoolError(sqlx::Error),
    InsertSubscriberError(sqlx::Error),
    TransactionCommitError(sqlx::Error),
}
impl From<String> for SubscribeError {
    fn from(value: String) -> Self {
        SubscribeError::ValidationError(value)
    }
}
impl From<SaveTokenError> for SubscribeError {
    fn from(value: SaveTokenError) -> Self {
        Self::SaveTokenError(value)
    }
}
impl From<reqwest::Error> for SubscribeError {
    fn from(value: reqwest::Error) -> Self {
        Self::SendEmailError(value)
    }
}
impl std::fmt::Display for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscribeError::ValidationError(e) => write!(f, "{}", e),
            // 我们应该在这里怎么做？
            SubscribeError::SaveTokenError(_) => write!(f, "无法存储新订阅者的确认令牌。"),
            SubscribeError::SendEmailError(_) => {
                write!(f, "发送确认电子邮件失败。")
            },
            SubscribeError::PoolError(_) => {
                write!(f, "无法从连接池获取 Postgres 连接")
            }
            SubscribeError::InsertSubscriberError(_) => {
                write!(f, "无法将新订阅者插入数据库。")
            }
            SubscribeError::TransactionCommitError(_) => {
                write!(
                    f,
                    "无法提交 SQL 事务以存储新订阅者。"
                )
            }
        }
    }
}
impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl std::error::Error for SubscribeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SubscribeError::ValidationError(_) => None,
            SubscribeError::SaveTokenError(e) => Some(e),
            SubscribeError::SendEmailError(e) => Some(e),
            SubscribeError::PoolError(e) => Some(e),
            SubscribeError::InsertSubscriberError(e) => Some(e),
            SubscribeError::TransactionCommitError(e) => Some(e),
        }
    }
}
impl ResponseError for SubscribeError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscribeError::ValidationError(_) => reqwest::StatusCode::BAD_REQUEST,
            SubscribeError::PoolError(_)
            | SubscribeError::InsertSubscriberError(_)
            | SubscribeError::TransactionCommitError(_)
            | SubscribeError::SaveTokenError(_)
            | SubscribeError::SendEmailError(_) => reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
