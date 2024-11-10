use anyhow::Context;
use secrecy::{ExposeSecret, Secret};
use sqlx::{PgPool, Pool, Postgres};
use argon2::{password_hash::SaltString, Argon2,PasswordVerifier,PasswordHasher,PasswordHash};
#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
// 验证用户身份必须存在于数据库中，且密码正确。
pub async fn validate_credentials(
    credentials: &Credentials,
    pool: &Pool<Postgres>,
) -> Result<uuid::Uuid, AuthError> {
    let userinfo = get_stored_credentials(pool, &credentials.username).await?;

    let expect_pwd = match &userinfo {
        Some(v) => v.1.to_owned(),
        None => Secret::new(
            "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
                .to_owned(),
        ),
    };

    // PHC 字符串格式：
    // # ${algorithm}${algorithm version}${$-separated algorithm parameters}${hash}${salt}
    let pwd = credentials.password.to_owned();

    let curr_span = tracing::span::Span::current();
    tokio::task::spawn_blocking(move || {
        curr_span.in_scope(|| verify_password_hash(expect_pwd, pwd))
    })
    .await
    .context("Failed to spawn blocking task.")
    .map_err(AuthError::UnexpectedError)??;

    userinfo
        .ok_or(AuthError::InvalidCredentials(anyhow::anyhow!(
            "Invalid username."
        )))
        .map(|v| v.0)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>, //  现在拥有所有权
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format.")?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    pool: &Pool<Postgres>,
    username: &str,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, AuthError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        user_id: uuid::Uuid,
        password_hash: String,
    }

    let row: Option<Row> = sqlx::query_as(
        r#"
        select 
        user_id,password_hash
        from users
        where username=$1;
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .context("Failed to query user credentials from database.")?;

    Ok(row.map(|v| (v.user_id, Secret::new(v.password_hash))))
}

#[tracing::instrument(
    name = "Change password",
    skip(pool, user_id),
    fields(user_id = %user_id)
)]
pub async fn change_password(
    user_id: uuid::Uuid,
    password: Secret<String>,
    pool: &PgPool,
) -> Result<(), anyhow::Error> {
    let pwd_hash=tokio::task::spawn_blocking(||{
        compute_password_hash(password)
    }).await?
    .context("Failed to hash password.")?;

    sqlx::query(r#"
        UPDATE users
        SET password_hash = $1
        WHERE user_id = $2
    "#)
    .bind(pwd_hash.expose_secret())
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update password.")?;

    Ok(())
}

fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());

    let pwd_hash = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)
    .unwrap()
    .to_string();

    Ok(Secret::new(pwd_hash))
}
