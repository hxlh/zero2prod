use crate::{
    authentication::{validate_credentials, AuthError, Credentials, UserId},
    routes::admin::dashboard::get_name,
    util::{e500, see_other, see_other_with_flash_message},
};
use actix_web::{error::InternalError, web, HttpResponse};
use secrecy::{ExposeSecret, Secret};

#[derive(Debug, serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    pool: web::Data<sqlx::PgPool>,
    form: web::Form<FormData>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
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

    crate::authentication::change_password(&user_id, form.new_password.clone(), &pool)
        .await
        .map_err(e500)?;

    Ok(see_other_with_flash_message(
        "Your password has been changed.",
        "/admin/password",
        "/",
    ))
}

