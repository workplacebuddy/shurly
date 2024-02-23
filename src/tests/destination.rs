use axum::http::StatusCode;

use crate::tests::helper;

#[sqlx::test]
async fn test_destination(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "";
    let url = "https://www.example.com/";

    // create destination
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let existing_destination_id = destination.unwrap().id;

    // check root redirect
    let (status_code, location, _) = helper::root(&mut app, slug).await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(url.to_string()), location);

    // try to create with same slug
    let (status_code, destination, error) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(destination.is_none());
    assert_eq!(Some("Slug already exists".to_string()), error);

    // delete destination
    let (status_code, _) =
        helper::myabe_delete_destination(&mut app, &access_token, &existing_destination_id).await;
    assert_eq!(StatusCode::NO_CONTENT, status_code);

    // check root redirect
    let (status_code, location, _) = helper::root(&mut app, slug).await;
    assert_eq!(StatusCode::GONE, status_code);
    assert_eq!(None, location);

    // try to create with same slug
    let (status_code, destination, error) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(destination.is_none());
    assert_eq!(
        Some("Slug already exists and is deleted".to_string()),
        error
    );
}
