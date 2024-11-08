use actix_web::{http::header::ContentType, web, HttpResponse};
use hmac::Mac;
use secrecy::{ExposeSecret, Secret};

#[derive(serde::Deserialize)]
pub struct QueryParams {
    error: String,
    tag: String,
}
impl QueryParams {
    fn verify(&self, hmac_secret: &Secret<String>) -> Result<String, anyhow::Error> {
        let tag = hex::decode(&self.tag)?;
        let query_string = format!("error={}", urlencoding::Encoded::new(&self.error));
        let mut mac =
            hmac::Hmac::<sha2::Sha256>::new_from_slice(hmac_secret.expose_secret().as_bytes())?;

        mac.update(query_string.as_bytes());
        mac.verify_slice(&tag)?;
        Ok(self.error.clone())
    }
}

pub async fn login_from(
    query: Option<web::Query<QueryParams>>,
    hmac_secret: web::Data<Secret<String>>,
) -> HttpResponse {
    let error_html = match query {
        None => "".into(),
        Some(q) => match q.verify(&hmac_secret) {
            Ok(error) => {
                format!("<p><i>{}</i></p>", htmlescape::encode_minimal(&error))
            }
            Err(e) => {
                tracing::warn!(
                    error.message = %e,
                    error.cause_chain = ?e,
                    "Failed to verify query parameters using the HMAC tag"
                );
                "".into()
            }
        },
    };
    tracing::info!("================================{}", error_html);
    HttpResponse::Ok()
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
        ))
}
