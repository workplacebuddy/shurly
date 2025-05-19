use axum::Router;
use axum::body::Body;
use axum::body::Bytes;
use axum::http::Method;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::header::CONTENT_TYPE;
use axum::http::header::LOCATION;
use http_body_util::BodyExt;
use serde_json::Map;
use serde_json::Value;
use tower::Service;
use uuid::Uuid;

use crate::database::DatabaseConfig;
use crate::setup_app;

/// Test helper version of User struct
#[derive(Debug)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[allow(dead_code)] // used by sqlx
    pub role: String,
    pub password: Option<String>,
}

/// Test helper version of Destination struct
#[derive(Debug)]
pub struct Destination {
    pub id: Uuid,
    pub slug: String,
    #[allow(dead_code)] // used by sqlx
    pub url: String,
}

/// Test helper version of Note struct
#[derive(Debug, PartialEq, Eq)]
pub struct Note {
    pub id: Uuid,
    pub content: String,
}

/// Error response
#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub error: String,
    pub description: Option<String>,
}

/// Setup the Shurly app
///
/// Inject some environment variables to match our tests
pub async fn setup_test_app(pool: sqlx::PgPool) -> Router {
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("INITIAL_USERNAME", "admin");
        std::env::set_var("INITIAL_PASSWORD", "verysecret");
        std::env::set_var("JWT_SECRET", "verysecret");
    }

    setup_app(DatabaseConfig::ExistingConnection(pool))
        .await
        .unwrap()
}

pub async fn root(app: &mut Router, slug: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/{slug}"))
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();

    let status_code = response.status();
    let headers = response.headers();

    let location = headers.get(LOCATION);
    let location = location.map(|header| header.to_str().unwrap().to_string());

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&body[..]).to_string();

    (status_code, location, body)
}

pub async fn login_with_password(app: &mut Router, password: &str) -> String {
    let mut payload = Map::new();
    payload.insert("username".to_string(), Value::String("admin".to_string()));
    payload.insert("password".to_string(), Value::String(password.to_string()));

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/users/token")
        .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    assert_eq!(StatusCode::OK, status_code);

    get_access_token(&body)
}

pub async fn login(app: &mut Router) -> String {
    login_with_password(app, "verysecret").await
}

pub async fn maybe_change_password(
    app: &mut Router,
    access_token: &str,
    current_password: &str,
    password: &str,
) -> (StatusCode, Option<String>, Option<String>) {
    let mut payload = Map::new();
    payload.insert(
        "currentPassword".to_string(),
        Value::String(current_password.to_string()),
    );
    payload.insert("password".to_string(), Value::String(password.to_string()));

    let request = Request::builder()
        .method(Method::PUT)
        .uri("/api/users/me/password")
        .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .header(AUTHORIZATION, access_token)
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_access_token(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn single_destination(
    app: &mut Router,
    access_token: &str,
    id: &Uuid,
) -> (StatusCode, Option<Destination>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/destinations/{id}"))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_destination(&body))
        } else {
            None
        },
    )
}

pub async fn list_destinations(
    app: &mut Router,
    access_token: &str,
) -> (StatusCode, Option<Vec<Destination>>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/destinations")
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_destinations(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_create_destination_with_is_permanent(
    app: &mut Router,
    access_token: &str,
    slug: &str,
    url: &str,
    is_permanent: bool,
) -> (StatusCode, Option<Destination>, Option<String>) {
    let mut payload = Map::new();
    payload.insert("slug".to_string(), Value::String(slug.to_string()));
    payload.insert("url".to_string(), Value::String(url.to_string()));
    payload.insert("isPermanent".to_string(), Value::Bool(is_permanent));

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/destinations")
        .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .header(AUTHORIZATION, access_token)
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::CREATED {
            Some(get_destination(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_create_destination(
    app: &mut Router,
    access_token: &str,
    slug: &str,
    url: &str,
) -> (StatusCode, Option<Destination>, Option<String>) {
    maybe_create_destination_with_is_permanent(app, access_token, slug, url, false).await
}

pub async fn maybe_create_destination_with_raw_body(
    app: &mut Router,
    access_token: &str,
    body: &'static str,
    include_content_type: bool,
) -> (StatusCode, Option<Destination>, Option<Error>) {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri("/api/destinations");

    if include_content_type {
        builder = builder.header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref());
    }

    let request = builder
        .header(AUTHORIZATION, access_token)
        .body(Body::from(body.as_bytes()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::CREATED {
            Some(get_destination(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_update_destination(
    app: &mut Router,
    access_token: &str,
    destination_id: &Uuid,
    url: &str,
) -> (StatusCode, Option<String>) {
    let mut payload = Map::new();
    payload.insert("url".to_string(), Value::String(url.to_string()));
    payload.insert("isPermanent".to_string(), Value::Bool(false));

    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/destinations/{destination_id}"))
        .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .header(AUTHORIZATION, access_token)
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn myabe_delete_destination(
    app: &mut Router,
    access_token: &str,
    id: &Uuid,
) -> (StatusCode, Option<String>) {
    let request = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/destinations/{id}"))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_create_note(
    app: &mut Router,
    access_token: &str,
    destination_id: &Uuid,
    content: &str,
) -> (StatusCode, Option<Note>, Option<String>) {
    let mut payload = Map::new();
    payload.insert("content".to_string(), Value::String(content.to_string()));

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/destinations/{destination_id}/notes"))
        .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .header(AUTHORIZATION, access_token)
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::CREATED {
            Some(get_note(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn list_notes(
    app: &mut Router,
    access_token: &str,
    destination_id: &Uuid,
) -> (StatusCode, Option<Vec<Note>>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/destinations/{destination_id}/notes"))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_notes(&body))
        } else {
            None
        },
    )
}

pub async fn single_note(
    app: &mut Router,
    access_token: &str,
    destination_id: &Uuid,
    note_id: &Uuid,
) -> (StatusCode, Option<Note>, Option<String>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/destinations/{destination_id}/notes/{note_id}",
        ))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_note(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST || status_code == StatusCode::NOT_FOUND {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn single_note_with_str(
    app: &mut Router,
    access_token: &str,
    destination_id: &Uuid,
    note_id: &str,
) -> (StatusCode, Option<Note>, Option<String>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/destinations/{destination_id}/notes/{note_id}",
        ))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_note(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST || status_code == StatusCode::NOT_FOUND {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_update_note(
    app: &mut Router,
    access_token: &str,
    destination_id: &Uuid,
    note_id: &Uuid,
    content: &str,
) -> (StatusCode, Option<Note>, Option<String>) {
    let mut payload = Map::new();
    payload.insert("content".to_string(), Value::String(content.to_string()));

    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!(
            "/api/destinations/{destination_id}/notes/{note_id}",
        ))
        .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .header(AUTHORIZATION, access_token)
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_note(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn myabe_delete_note(
    app: &mut Router,
    access_token: &str,
    destination_id: &Uuid,
    note_id: &Uuid,
) -> (StatusCode, Option<String>) {
    let request = Request::builder()
        .method(Method::DELETE)
        .uri(format!(
            "/api/destinations/{destination_id}/notes/{note_id}",
        ))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn current_user(app: &mut Router, access_token: &str) -> (StatusCode, Option<User>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/users/me")
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_user(&body))
        } else {
            None
        },
    )
}

pub async fn single_user(
    app: &mut Router,
    access_token: &str,
    id: &Uuid,
) -> (StatusCode, Option<User>, Option<String>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/users/{id}"))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_user(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST || status_code == StatusCode::NOT_FOUND {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_delete_user(
    app: &mut Router,
    access_token: &str,
    id: &Uuid,
) -> (StatusCode, Option<String>) {
    let request = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/users/{id}"))
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::BAD_REQUEST || status_code == StatusCode::NOT_FOUND {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn list_users(app: &mut Router, access_token: &str) -> (StatusCode, Option<Vec<User>>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/users")
        .header(AUTHORIZATION, access_token)
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::OK {
            Some(get_users(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_create_user_with_password(
    app: &mut Router,
    access_token: &str,
    username: &str,
    role: &str,
    password: Option<&str>,
) -> (StatusCode, Option<User>, Option<String>) {
    let mut payload = Map::new();
    payload.insert("username".to_string(), Value::String(username.to_string()));
    payload.insert("role".to_string(), Value::String(role.to_string()));

    if let Some(password) = password {
        payload.insert("password".to_string(), Value::String(password.to_string()));
    }

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/users")
        .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .header(AUTHORIZATION, access_token)
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.call(request).await.unwrap();
    let status_code = response.status();

    let body = response.into_body().collect().await.unwrap().to_bytes();

    (
        status_code,
        if status_code == StatusCode::CREATED {
            Some(get_user(&body))
        } else {
            None
        },
        if status_code == StatusCode::BAD_REQUEST {
            Some(get_error_message(&body))
        } else {
            None
        },
    )
}

pub async fn maybe_create_user(
    app: &mut Router,
    access_token: &str,
    username: &str,
    role: &str,
) -> (StatusCode, Option<User>, Option<String>) {
    maybe_create_user_with_password(app, access_token, username, role, None).await
}

fn value_to_user(user: &Map<String, Value>) -> User {
    User {
        id: user["id"].as_str().map(Uuid::parse_str).unwrap().unwrap(),
        username: user["username"].as_str().map(ToString::to_string).unwrap(),
        role: user["role"].as_str().map(ToString::to_string).unwrap(),
        password: user
            .get("password")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

fn get_user(body: &Bytes) -> User {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["data"]
        .as_object()
        .map(value_to_user)
        .unwrap()
}

fn get_users(body: &Bytes) -> Vec<User> {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_object().unwrap())
        .map(value_to_user)
        .collect()
}

fn value_to_destination(destination: &Map<String, Value>) -> Destination {
    Destination {
        id: destination["id"]
            .as_str()
            .map(Uuid::parse_str)
            .unwrap()
            .unwrap(),
        slug: destination["slug"]
            .as_str()
            .map(ToString::to_string)
            .unwrap(),
        url: destination["url"]
            .as_str()
            .map(ToString::to_string)
            .unwrap(),
    }
}

fn get_destination(body: &Bytes) -> Destination {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["data"]
        .as_object()
        .map(value_to_destination)
        .unwrap()
}

fn get_destinations(body: &Bytes) -> Vec<Destination> {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_object().unwrap())
        .map(value_to_destination)
        .collect()
}

fn value_to_note(note: &Map<String, Value>) -> Note {
    Note {
        id: note["id"].as_str().map(Uuid::parse_str).unwrap().unwrap(),
        content: note["content"].as_str().map(ToString::to_string).unwrap(),
    }
}

fn get_note(body: &Bytes) -> Note {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["data"]
        .as_object()
        .map(value_to_note)
        .unwrap()
}

fn get_notes(body: &Bytes) -> Vec<Note> {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_object().unwrap())
        .map(value_to_note)
        .collect()
}

fn value_to_error(error: &Map<String, Value>) -> Error {
    Error {
        error: error["error"].as_str().map(ToString::to_string).unwrap(),
        description: error
            .get("description")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

fn get_error(body: &Bytes) -> Error {
    serde_json::from_slice::<Value>(&body[..])
        .unwrap()
        .as_object()
        .map(value_to_error)
        .unwrap()
}

fn get_error_message(body: &Bytes) -> String {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["error"]
        .as_str()
        .map(ToString::to_string)
        .unwrap()
}

fn get_access_token(body: &Bytes) -> String {
    serde_json::from_slice::<Value>(&body[..]).unwrap()["data"]["access_token"]
        .as_str()
        .map(|access_token| format!("Bearer {access_token}"))
        .unwrap()
}
