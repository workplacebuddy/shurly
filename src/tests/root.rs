use axum::http::StatusCode;

use crate::tests::helper;

#[tokio::test]
async fn test_root() {
    let mut app = helper::setup_test_app().await;

    let (status_code, location) = helper::root(&mut app, "").await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);
    assert_eq!(None, location);
}
