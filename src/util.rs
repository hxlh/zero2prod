use actix_web::HttpResponse;
use actix_web::http::header::LOCATION;

// 返回一个不透明的 500，同时保留错误的根本原因以进行日志记录。
pub fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}

pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}