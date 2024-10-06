use actix_web::{web, HttpResponse, Responder};

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};

#[derive(serde::Deserialize)]
pub struct SubscriptionsData {
    name: String,
    email: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    ),
)]
pub async fn subscriptions(
    form: web::Form<SubscriptionsData>,
    pool: web::Data<sqlx::Pool<sqlx::Sqlite>>,
) -> impl Responder {
    let name = match SubscriberName::parse(form.0.name) {
        Ok(v) => v,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };
    let email = match SubscriberEmail::parse(form.0.email) {
        Ok(v) => v,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let new_subscriber = NewSubscriber {
        name: name,
        email: email,
    };
    match save_subscriber(&new_subscriber, &pool).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database.",
    skip(subscriber, pool)
)]
async fn save_subscriber(
    subscriber: &NewSubscriber,
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().timestamp();
    let subscriber_email = subscriber.email.as_ref();
    let subscriber_name = subscriber.name.as_ref();

    sqlx::query!(
        r#"
        INSERT INTO subscriptions (email, name, subscribed_at)
        Values (?,?,?)
        "#,
        subscriber_email,
        subscriber_name,
        now,
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(
            "Failed to save new subscriber details in the database: {}",
            e
        );
        e
    })?;
    Ok(())
}
