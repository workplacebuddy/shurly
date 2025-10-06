use axum::http::StatusCode;

use crate::tests::helper;

#[sqlx::test]
async fn test_destination_forward_query_param(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "some-slug";
    let url = "https://www.example.com/";

    // create destination with slug
    let (status_code, destination, _) = helper::maybe_create_destination_with_forward_query_param(
        &mut app,
        &access_token,
        slug,
        url,
        true,
    )
    .await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());

    let destination = destination.unwrap();
    assert!(destination.forward_query_parameters);

    let (status_code, location, _) = helper::root_with_query(&mut app, slug, "?foo=bar").await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(format!("{url}?foo=bar")), location);

    // setup
    let slug = "other-slug";
    let url = "https://www.example.com/something?page=1";

    // create destination with slug
    let (status_code, destination, _) = helper::maybe_create_destination_with_forward_query_param(
        &mut app,
        &access_token,
        slug,
        url,
        true,
    )
    .await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());

    let (status_code, location, _) = helper::root_with_query(&mut app, slug, "?foo=bar").await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(format!("{url}&foo=bar")), location);

    let (status_code, location, _) = helper::root_with_query(&mut app, slug, "?page=2").await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(url.to_string()), location);

    let (status_code, location, _) =
        helper::root_with_query(&mut app, slug, "?page=2&page=3").await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(url.to_string()), location);
}

#[sqlx::test]
async fn test_destination_without_forward_query_param(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "some-slug";

    let url = "https://www.example.com/";

    // create destination with empty slug
    let (status_code, destination, _) = helper::maybe_create_destination_with_forward_query_param(
        &mut app,
        &access_token,
        slug,
        url,
        false,
    )
    .await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());

    let (status_code, location, _) = helper::root_with_query(&mut app, slug, "?foo=bar").await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(url.to_string()), location);
}
