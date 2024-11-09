use actix_web::{cookie, http::header::ContentType, HttpRequest, HttpResponse};

pub async fn login_from(request: HttpRequest) -> HttpResponse {
    &dbg!(&request.cookies());
    let error_html: String = match request.cookie("_flash") {
        Some(cookie) => {
            format!(r#"<p><i>{}</i></p>"#, cookie.value())
        }
        None => "".to_string(),
    };

    let mut resp = HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>
<body>
    {error_html}
    <form action="/login" method="post">
        <label>Username
            <input
                type="text"
                placeholder="Enter Username"
                name="username"
            >
        </label>
        <label>Password
            <input
                type="password"
                placeholder="Enter Password"
                name="password"
            >
        </label>
        <button type="submit">Login</button>
    </form>
</body>
</html>"#,
        ));

    if !error_html.is_empty() {
        resp.add_removal_cookie(&cookie::Cookie::new("_flash", ""))
            .unwrap();
    }

    resp
}
