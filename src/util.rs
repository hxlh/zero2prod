use actix_web::cookie::Cookie;
use actix_web::http::header::LOCATION;
use actix_web::HttpResponse;

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

pub fn see_other_with_flash_message(message: &str, location: &str, path: &str) -> HttpResponse {
    let mut cookie = Cookie::new("_flash", message);
    cookie.set_path(path);

    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .cookie(cookie)
        .finish()
}
