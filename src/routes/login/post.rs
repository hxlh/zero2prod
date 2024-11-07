use actix_web::{web, HttpResponse, ResponseError};
use reqwest::{header::LOCATION, StatusCode};
use secrecy::Secret;
use sqlx::{Pool, Postgres};

use crate::{
    authentication::{validate_credentials, Credentials},
    routes::error_chain_fmt,
};

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    name = "login",
    skip(from),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    pool: web::Data<Pool<Postgres>>,
    from: web::Form<FormData>,
) -> Result<HttpResponse, LoginError> {
    let credentials = Credentials {
        username: from.0.username,
        password: from.0.password,
    };
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    let user_id = validate_credentials(&credentials, &pool)
        .await
        .map_err(|e| match e {
            crate::authentication::AuthError::InvalidCredentials(error) => {
                LoginError::AuthError(error)
            }
            crate::authentication::AuthError::UnexpectedError(error) => {
                LoginError::UnexpectedError(error)
            }
        })?;

    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    // 重定向 303 See Other
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/"))
        .finish())
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Unexpected error")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl ResponseError for LoginError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let encoded_error = urlencoding::Encoded::new(self.to_string());
        HttpResponse::build(self.status_code())
            .insert_header((LOCATION, format!("/login?error={}", encoded_error)))
            .finish()
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::SEE_OTHER
        // match self {
        //     LoginError::AuthError(_) => StatusCode::UNAUTHORIZED,
        //     LoginError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        // }
    }
}
