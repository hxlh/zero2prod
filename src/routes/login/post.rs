use actix_session::Session;
use actix_web::{
    cookie::{self, Cookie},
    error::InternalError,
    web, HttpResponse,
};
use secrecy::Secret;
use sqlx::{Pool, Postgres};

use crate::{
    authentication::{validate_credentials, Credentials},
    routes::{error_chain_fmt, PublishError},
};

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    name = "login",
    skip(pool,from,secret,session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    pool: web::Data<Pool<Postgres>>,
    from: web::Form<FormData>,
    secret: web::Data<Secret<String>>,
    session: Session,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: from.0.username,
        password: from.0.password,
    };
    tracing::Span::current().record("username", tracing::field::display(&credentials.username));

    match validate_credentials(&credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(&user_id));
            // 重定向
            session.renew();
            if let Err(e) = session.insert("user_id", user_id) {
                let e = LoginError::UnexpectedError(e.into());

                let mut flash_error = Cookie::new("_flash", e.to_string());
                flash_error.set_expires(Some(
                    cookie::time::OffsetDateTime::now_utc() + cookie::time::Duration::seconds(5),
                ));

                let response = HttpResponse::SeeOther()
                    .insert_header((
                        reqwest::header::LOCATION,
                        // format!("/login?{}&tag={:x}", query_string, hmac_tag),
                        "/login",
                    ))
                    .cookie(flash_error)
                    .finish();
                return Err(InternalError::from_response(e, response));
            }
            Ok(HttpResponse::SeeOther()
                .insert_header((reqwest::header::LOCATION, "/admin/dashboard"))
                .finish())
        }
        Err(e) => {
            let e = match e {
                crate::authentication::AuthError::InvalidCredentials(_) => {
                    LoginError::AuthError(e.into())
                }
                crate::authentication::AuthError::UnexpectedError(_) => {
                    LoginError::UnexpectedError(e.into())
                }
            };

            // let query_string = format!("error={}", urlencoding::Encoded::new(e.to_string()));
            // let _hmac_tag = {
            //     let mut mac =
            //         Hmac::<sha2::Sha256>::new_from_slice(secret.expose_secret().as_bytes())
            //             .unwrap();
            //     mac.update(query_string.as_bytes());
            //     mac.finalize().into_bytes()
            // };
            let mut flash_error = Cookie::new("_flash", e.to_string());
            flash_error.set_expires(Some(
                cookie::time::OffsetDateTime::now_utc() + cookie::time::Duration::seconds(5),
            ));

            let response = HttpResponse::SeeOther()
                .insert_header((
                    reqwest::header::LOCATION,
                    // format!("/login?{}&tag={:x}", query_string, hmac_tag),
                    "/login",
                ))
                .cookie(flash_error)
                .finish();
            Err(InternalError::from_response(e, response))
        }
    }
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
