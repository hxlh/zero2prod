use crate::{domain::SubscriberEmail, email_client, routes::error_chain_fmt};
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use reqwest::StatusCode;
use sqlx::{prelude::FromRow, Pool, Postgres, Transaction};

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: BodyContext,
}
#[derive(serde::Deserialize)]
pub struct BodyContext {
    html: String,
    text: String,
}

pub async fn publish_newsletter(
    pool: web::Data<Pool<Postgres>>,
    email_client: web::Data<email_client::EmailClient>,
    body: web::Json<BodyData>,
) -> Result<HttpResponse, PublishError> {
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;
    let subscribers = get_confirmed_subscribers(&mut tx).await?;

    for subscriber in subscribers {
        match subscriber {
            Ok(s) => {
                email_client
                    .send(
                        &s.email,
                        &body.title,
                        &body.content.text,
                        &body.content.html,
                    )
                    .await
                    .with_context(|| format!("Failed to send email to {}", &s.email))?;
            }
            Err(e) => {
                tracing::warn!(
                    // 我们将错误链记录为日志记录上的结构化字段。
                    e.cause_chain = ?e,
                    // 使用 `\` 将长字符串字面量分成两行，而不创建 `\n` 字符。
                    "Skipping a confirmed subscriber. \
                     Their stored contact details are invalid",
                );
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(tx))]
async fn get_confirmed_subscribers(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    
    let subscribers = sqlx::query_as(
        r#"
        select email from subscriptions where status='confirmed';
        "#,
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|r: (String,)| match SubscriberEmail::parse(r.0) {
        Ok(v) => Ok(ConfirmedSubscriber { email: v }),
        Err(e) => Err(anyhow::anyhow!(e)),
    })
    .collect();

    Ok(subscribers)
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

// 相同的逻辑以在 `Debug` 上获取完整的错误链
impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
