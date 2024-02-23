use axum::http::StatusCode;
use percent_encoding::utf8_percent_encode;
use percent_encoding::NON_ALPHANUMERIC;

use crate::tests::helper;

#[sqlx::test]
async fn test_emoji_slug(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "🦙";
    let url = "https://www.example.com/";

    // create destination with emoji slug
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let destination_id = destination.unwrap().id;

    // verify
    let (status_code, destination) =
        helper::single_destination(&mut app, &access_token, &destination_id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(destination.is_some());

    // emojis are encoded with percent encoding in URLs
    let encoded_slug = utf8_percent_encode(slug, NON_ALPHANUMERIC).to_string();

    let (status_code, _, _) = helper::root(&mut app, &encoded_slug).await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
}
