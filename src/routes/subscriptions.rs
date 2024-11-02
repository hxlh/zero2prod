use std::fmt::write;

use actix_web::{web, HttpResponse, Responder, ResponseError};
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{Pool, Postgres, Row, Transaction};
use tracing_log::log::{self};
use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
};

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
) -> Result<HttpResponse,actix_web::Error> {
    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return Ok(HttpResponse::BadRequest().finish()),
    };

    let mut tx=match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            log::error!("Failed to begin transaction: {}", e);
            return Ok(HttpResponse::BadRequest().finish());
        },
    };

    // 保存订阅者信息
    let id = match save_subscriber(&mut tx,&new_subscriber).await {
        Ok(id) => id,
        Err(_) =>return Ok(HttpResponse::BadRequest().finish()),
    };
    // 生成token并保存,同时生成订阅确认链接并发送给用户
    let subscription_token=generate_subscription_token();
    save_subscription_token(&mut tx, id, &subscription_token).await?;

    if let Err(e) = send_confirmation_email(
        email_client.as_ref(),
        new_subscriber,
        &base_url,
        &subscription_token,
    )
    .await
    {
        log::error!("Failed to send confirmation email: {}", e);
        return Ok(HttpResponse::BadRequest().finish());
    }

    if tx.commit().await.is_err() {
        log::error!("Failed to commit transaction");
        return Ok(HttpResponse::BadRequest().finish());
    }

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

    let row=sqlx::query(
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
)->Result<(), SaveTokenError> {
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
    .map_err(|e|{
        tracing::error!("Failed to execute query: {:?}",e);
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

impl ResponseError for SaveTokenError {}

impl std::fmt::Display for SaveTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to save subscription token in the database"
        )
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


