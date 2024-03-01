//! All API endpoint setup

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

mod audit_trail;
mod current_user;
mod destinations;
mod notes;
mod request;
mod response;
mod users;

/// Get the Axum router for all API routes
pub fn router() -> Router {
    let users = Router::new()
        .route("/token", post(users::token))
        .route("/", get(users::list))
        .route("/", post(users::create))
        .route("/me/password", put(users::change_password))
        .route("/:user/password", put(users::change_password))
        .route("/me", get(users::single))
        .route("/:user", get(users::single))
        .route("/:user", delete(users::delete));

    let notes = Router::new()
        .route("/", get(notes::list))
        .route("/", post(notes::create))
        .route("/:note", get(notes::single))
        .route("/:note", patch(notes::update))
        .route("/:note", delete(notes::delete));

    let destinations = Router::new()
        .route("/", get(destinations::list))
        .route("/", post(destinations::create))
        .route("/:destination", get(destinations::single))
        .route("/:destination", patch(destinations::update))
        .route("/:destination", delete(destinations::delete))
        .nest("/:destination/notes", notes);

    Router::new()
        .nest("/users", users)
        .nest("/destinations", destinations)
}
