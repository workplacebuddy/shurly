use axum::routing::delete;
use axum::routing::get;
use axum::routing::patch;
use axum::routing::post;
use axum::routing::put;
use axum::Router;

pub use audit_trail::AuditTrail;
pub use current_user::CurrentUser;
pub use current_user::JwtKeys;
pub use request::parse_slug;
pub use request::parse_url;
pub use request::Form;
pub use request::PathParameters;
pub use response::Error;
pub use response::Success;

use crate::storage::Storage;

mod audit_trail;
mod current_user;
mod destinations;
mod notes;
mod request;
mod response;
mod users;

pub fn router<S: Storage>() -> Router {
    let users = Router::new()
        .route("/token", post(users::token::<S>))
        .route("/", get(users::list::<S>))
        .route("/", post(users::create::<S>))
        .route("/me/password", put(users::change_password::<S>))
        .route("/:user/password", put(users::change_password::<S>))
        .route("/:user", get(users::single::<S>))
        .route("/:user", delete(users::delete::<S>));

    let notes = Router::new()
        .route("/", get(notes::list::<S>))
        .route("/", post(notes::create::<S>))
        .route("/:note", get(notes::single::<S>))
        .route("/:note", patch(notes::update::<S>))
        .route("/:note", delete(notes::delete::<S>));

    let destinations = Router::new()
        .route("/", get(destinations::list::<S>))
        .route("/", post(destinations::create::<S>))
        .route("/:destination", get(destinations::single::<S>))
        .route("/:destination", patch(destinations::update::<S>))
        .route("/:destination", delete(destinations::delete::<S>))
        .nest("/:destination/notes", notes);

    Router::new()
        .nest("/users", users)
        .nest("/destinations", destinations)
}
