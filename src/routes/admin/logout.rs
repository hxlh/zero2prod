use actix_session::Session;
use actix_web::{cookie::{Cookie, SameSite}, HttpResponse};
use uuid::Uuid;

use crate::util::{e500, see_other};

pub async fn log_out(session: Session) -> Result<HttpResponse, actix_web::Error> {
    if session.get::<Uuid>("user_id").map_err(e500)?.is_none() {
        return Ok(see_other("/login"));
    } else {
        session.purge();

        let mut flash_err=Cookie::new("_flash", "You have successfully logged out.");
        flash_err.set_same_site(SameSite::Lax);
        flash_err.set_path("/");

        let mut resp = see_other("/login");
        resp.add_cookie(&flash_err)?;

        Ok(resp)
    }
}
