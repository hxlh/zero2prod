use actix_web::{web, HttpResponse};
use sqlx::{PgPool, Pool, Postgres, Row};
use tracing_log::log;

#[derive(serde::Deserialize)]
pub struct Parameters {
    confirm_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(_parameters))]
pub async fn confirm(
    pool: web::Data<Pool<Postgres>>,
    _parameters: web::Query<Parameters>,
) -> HttpResponse {
    // 获取数据库池的引用；
    // 检索与令牌关联的订阅者 ID（如果存在）；
    // 将订阅者状态更改为已确认。
    let subscriber_id = match get_subscriber_id_by_token(&pool, &_parameters.confirm_token).await {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::InternalServerError().finish();
        }
    };

    if confirm_subscriber(&pool, subscriber_id).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(pool, subscriber_id))]
async fn confirm_subscriber(pool: &PgPool, subscriber_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        update subscriptions
        set status = 'confirmed'
        where id = $1
        "#,
    )
    .bind(subscriber_id)
    .execute(pool)
    .await
    .map_err(|e| {
        log::error!("Failed to confirm subscriber: {:?}", e);
        e
    })?;

    Ok(())
}

#[tracing::instrument(name = "Get subscriber ID by token", skip(pool, token))]
async fn get_subscriber_id_by_token(pool: &PgPool, token: &str) -> Result<i64, sqlx::Error> {
    let row = sqlx::query(
        r#"
        select subscriber_id 
        from subscription_tokens
        where subscription_token = $1 ;
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .map_err(|e|{
        log::error!("Failed to get subscriber ID by token: {:?}", e);
        e
    })?
    .ok_or(sqlx::Error::RowNotFound)
    .map_err(|e|{
        log::error!("Failed to get subscriber ID by token: {:?}", e);
        e
    })?;

    Ok(row.get(0))
}
