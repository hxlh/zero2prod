use crate::{
    authentication::{validate_credentials, AuthError, Credentials},
    routes::admin::dashboard::get_name,
    util::{e500, see_other, see_other_with_flash_message},
};
use actix_session::Session;
use actix_web::{cookie::Cookie, error::InternalError, web, HttpResponse};
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

#[derive(Debug, serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    session: Session,
    pool: web::Data<sqlx::PgPool>,
    form: web::Form<FormData>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = match session.get::<Uuid>("user_id").map_err(e500)? {
        Some(user_id) => user_id,
        None => return Ok(see_other("/login")),
    };

    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        return Ok(see_other_with_flash_message(
            "You entered two different new passwords - the field values must match.",
            "/admin/password",
            "/",
        ));
    }

    let username = get_name(&pool, &user_id).await.map_err(e500)?;

    let credentials = Credentials {
        username,
        password: form.current_password.clone(),
    };

    if let Err(e) = validate_credentials(&credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => Ok(see_other_with_flash_message(
                "The current password is incorrect.",
                "/admin/password",
                "/",
            )),
            AuthError::UnexpectedError(_) => Err(e500(e)),
        };
    };

    crate::authentication::change_password(user_id, form.new_password.clone(), &pool)
        .await
        .map_err(e500)?;

    Ok(see_other_with_flash_message(
        "Your password has been changed.",
        "/admin/password",
        "/",
    ))
}

async fn reject_anonymous_users(
    session: Session
) -> Result<Uuid, actix_web::Error> {
    match session.get::<Uuid>("user_id").map_err(e500)? {
        Some(user_id) => Ok(user_id),
        None => {
            let response = see_other("/login");
            let e = anyhow::anyhow!("The user has not logged in");
            Err(InternalError::from_response(e, response).into())
        }
    }
}
