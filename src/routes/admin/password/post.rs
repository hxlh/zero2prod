use crate::{
    authentication::{validate_credentials, AuthError, Credentials},
    routes::admin::dashboard::get_name,
    util::{e500, see_other},
};
use actix_session::Session;
use actix_web::{cookie::Cookie, web, HttpResponse};
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
        let resp = HttpResponse::SeeOther()
            .insert_header((reqwest::header::LOCATION, "/admin/password"))
            .cookie(Cookie::new(
                "_flash",
                "You entered two different new passwords - the field values must match.",
            ))
            .finish();
        return Ok(resp);
    }

    let username = get_name(&pool, &user_id).await.map_err(e500)?;

    let credentials = Credentials {
        username,
        password: form.current_password.clone(),
    };

    if let Err(e) = validate_credentials(&credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                let resp = HttpResponse::SeeOther()
                    .insert_header((reqwest::header::LOCATION, "/admin/password"))
                    .cookie(Cookie::new("_flash", "The current password is incorrect."))
                    .finish();
                Ok(resp)
            }
            AuthError::UnexpectedError(_) => Err(e500(e)),
        };
    };

    Ok(HttpResponse::Ok().finish())
}
