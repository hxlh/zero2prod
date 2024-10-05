use actix_web::{web, HttpResponse, Responder};

#[derive(serde::Deserialize)]
pub struct SubscriptionsData {
    name:String,
    email:String
}

pub async fn subscriptions(form: web::Form<SubscriptionsData>) -> impl Responder {
    HttpResponse::Ok().finish()
}
