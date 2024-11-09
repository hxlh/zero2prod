use actix_session::Session;
use actix_web::{http::header::ContentType, web, HttpResponse};
use anyhow::Context;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

// 返回一个不透明的 500，同时保留错误的根本原因以进行日志记录。
fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}

pub async fn admin_dashboard(
    session: Session,
    pool:web::Data<Pool<Postgres>>,
) -> Result<HttpResponse, actix_web::Error>{
    let username = if let Some(user_id) = session.get::<Uuid>("user_id").map_err(e500)? {
        get_name(&pool,&user_id).await
        .map_err(e500)?
    }else{
        return Err(e500(anyhow::anyhow!("User not logged in")));
    };
 
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta http-equiv="content-type" content="text/html; charset=utf-8">
<title>Admin dashboard</title>
</head>
<body>
<p>Welcome {username}!</p>
</body>
</html>"#
        )))
}

async fn get_name(pool:&Pool<Postgres>,user_id: &Uuid) -> Result<String,anyhow::Error> {
    let username=sqlx::query_as::<_, (String,)>(
        r#"
    SELECT username
        FROM users
    WHERE user_id = $1
    "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Failed to fetch user name")?;

    Ok(username.0)
}
