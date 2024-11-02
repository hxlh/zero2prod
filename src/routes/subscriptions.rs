use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{Pool, Postgres, Row};
use tracing_log::log;

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
) -> impl Responder {
    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    // 保存订阅者信息
    let id = match save_subscriber(&new_subscriber, &pool).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    // 生成token并保存,同时生成订阅确认链接并发送给用户
    let subscription_token=generate_subscription_token();
    if save_subscription_token(&pool, id, &subscription_token).await.is_err(){
        return HttpResponse::InternalServerError().finish();
    }

    if let Err(e) = send_confirmation_email(
        email_client.as_ref(),
        new_subscriber,
        &base_url,
        &subscription_token,
    )
    .await
    {
        log::error!("Failed to send confirmation email: {}", e);
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
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
    skip(subscriber, pool)
)]
async fn save_subscriber(
    subscriber: &NewSubscriber,
    pool: &Pool<Postgres>,
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
    .fetch_optional(pool)
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
    skip(pool, id, token)
)]
async fn save_subscription_token(
    pool: &Pool<Postgres>,
    id: i64,
    token: &str,
)->Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        insert into subscription_tokens (subscriber_id, subscription_token)
        values ($1, $2)
        "#,
    )
    .bind(id)
    .bind(token)
    .execute(pool)
    .await
    .map_err(|e|{
        tracing::error!("Failed to execute query: {:?}",e);
        e
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
