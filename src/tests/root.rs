use axum::http::StatusCode;

use crate::tests::helper;

#[sqlx::test]
async fn test_root(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let (status_code, location, _) = helper::root(&mut app, "").await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);
    assert_eq!(None, location);
}

#[sqlx::test]
async fn test_root_with_valid_utf8(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let (status_code, location, _) = helper::root(&mut app, "%20").await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);
    assert_eq!(None, location);
}

#[sqlx::test]
async fn test_root_with_invalid_utf8(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let (status_code, location, body) = helper::root(&mut app, "%c0").await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert_eq!(None, location);
    assert!(body.contains("URL contains invalid UTF-8 characters"));
}
