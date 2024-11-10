use std::ops::Deref;

use actix_session::SessionExt;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::HttpMessage;
use actix_web_lab::middleware::Next;
use uuid::Uuid;
#[derive(Copy, Clone, Debug)]
pub struct UserId(Uuid);
impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl Deref for UserId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session = {
        let (http_request, _) = req.parts_mut();
        http_request.get_session()
    };

    if let Some(user_id) = session.get::<Uuid>("user_id").unwrap_or(None) {
        req.extensions_mut().insert(UserId(user_id));
        next.call(req).await
    } else {
        let resp = crate::util::see_other("/login");
        let e = anyhow::anyhow!("The user has not logged in");
        return Err(InternalError::from_response(e, resp).into());
    }
}
