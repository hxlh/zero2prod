use crate::{domain::SubscriberEmail, email_client, routes::error_chain_fmt};
use actix_web::{http::header::HeaderMap, web, HttpRequest, HttpResponse, ResponseError};
use anyhow::Context;
use base64::STANDARD;
use reqwest::StatusCode;
use secrecy::Secret;
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
    req: HttpRequest,
    pool: web::Data<Pool<Postgres>>,
    email_client: web::Data<email_client::EmailClient>,
    body: web::Json<BodyData>,
) -> Result<HttpResponse, PublishError> {
    let creditials = basic_authentication(req.headers())
    .map_err(|e|{
        PublishError::AuthError(e)
    })?;

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

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let auth=headers.get("Authorization")
    .context("Authorization header missing")?
    .to_str()
    .context("not only contains visible ASCII chars")?;

    let encode_segment=auth.strip_prefix("Basic ")
    .context("not a basic authentication header")?;

    let decode_bytes=base64::decode_config(encode_segment,STANDARD)
    .context("failed to decode base64 string")?;

    let credentials=std::str::from_utf8(&decode_bytes)
    .context("not a valid UTF-8 string")?;
    
    let mut credentials_iter=credentials.splitn(2,":");

    let username=credentials_iter
    .next() 
    .context("A username must be provided in 'Basic' auth.")?;
    let pwd=credentials_iter
    .next()
    .context("A password must be provided in 'Basic' auth.")?;

    Ok(Credentials {
        username: username.to_string(),
        password: Secret::new(pwd.to_string()),
    })
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
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
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
            PublishError::AuthError(_) => StatusCode::UNAUTHORIZED,
        }
    }
}
