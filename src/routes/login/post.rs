use actix_web::{web, HttpResponse};
use reqwest::header::LOCATION;
use secrecy::Secret;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

pub async fn login(from:web::Form<FormData>)->HttpResponse{
    // 重定向 303 See Other
    HttpResponse::SeeOther()
    .insert_header((LOCATION,"/"))
    .finish()
}