use axum::http::StatusCode;

use crate::tests::helper;

#[sqlx::test]
async fn test_destination_create(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let valid_empty_slug = "";
    let valid_non_empty_slug = "hello-world";
    let valid_with_slash_slug = "/2022/hello-world/";
    let valid_with_slash_slug_normalized = "2022/hello-world";
    let valid_with_url_encoded_slug = "hello world";
    let valid_with_url_encoded_slug_encoded = "hello%20world";
    let invalid_slug_one = "hello?world";
    let invalid_slug_two = "hello#world";

    let url = "https://www.example.com/";

    // create destination with empty slug
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, valid_empty_slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let valid_empty_destination_id = destination.unwrap().id;

    // verify
    let (status_code, destination) =
        helper::single_destination(&mut app, &access_token, &valid_empty_destination_id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(destination.is_some());

    // create destination with slug
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, valid_non_empty_slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let valid_non_empty_destination_id = destination.unwrap().id;

    // verify
    let (status_code, destination) =
        helper::single_destination(&mut app, &access_token, &valid_non_empty_destination_id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(destination.is_some());

    // create destination with slash slug
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, valid_with_slash_slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let valid_with_slash_destination_id = destination.unwrap().id;

    // verify, prefix and suffix `/`s are stripped
    let (status_code, destination) =
        helper::single_destination(&mut app, &access_token, &valid_with_slash_destination_id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(destination.is_some());
    assert_eq!(valid_with_slash_slug_normalized, &destination.unwrap().slug);

    // create destination with url encoded slug
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, valid_with_url_encoded_slug, url)
            .await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let valid_with_url_encoded_destination_id = destination.unwrap().id;

    // verify
    let (status_code, destination) = helper::single_destination(
        &mut app,
        &access_token,
        &valid_with_url_encoded_destination_id,
    )
    .await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(destination.is_some());

    let (status_code, _, _) = helper::root(&mut app, valid_with_url_encoded_slug_encoded).await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);

    // create destination with invalid slug
    let (status_code, _, error) =
        helper::maybe_create_destination(&mut app, &access_token, invalid_slug_one, url).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(error.is_some());
    assert_eq!(Some("Slug can not contain \"?\"".to_string()), error);

    // create destination with invalid slug
    let (status_code, _, error) =
        helper::maybe_create_destination(&mut app, &access_token, invalid_slug_two, url).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(error.is_some());
    assert_eq!(Some("Slug can not contain \"#\"".to_string()), error);
}
