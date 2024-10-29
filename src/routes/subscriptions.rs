use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use sqlx::{Pool, Postgres};
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
) -> impl Responder {
    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    if save_subscriber(&new_subscriber, &pool).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    if let Err(e) = send_confirmation_email(email_client.as_ref(), new_subscriber).await {
        log::error!("Failed to send confirmation email: {}", e);
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

#[tracing::instrument(name = "send confirmation email", skip(email_client, new_subscriber))]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
) -> Result<(), reqwest::Error> {
    let confirmation_link = "https://my-api.com/subscriptions/confirm";
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
) -> Result<(), sqlx::Error> {
    let subscriber_email = subscriber.email.as_ref();
    let subscriber_name = subscriber.name.as_ref();

    sqlx::query(
        r#"
        INSERT INTO subscriptions (email, name, subscribed_at,status)
        Values ($1,$2,$3,'pending_confirmation')
        "#,
    )
    .bind(subscriber_email)
    .bind(subscriber_name)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(
            "Failed to save new subscriber details in the database: {}",
            e
        );
        dbg!(&e);
        e
    })?;
    Ok(())
}
