use axum::http::StatusCode;

use crate::tests::helper;

#[sqlx::test]
async fn test_destination_update_is_permanent(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "";
    let url = "https://www.example.com/";

    // create destination
    let (status_code, destination, _) = helper::maybe_create_destination_with_is_permanent(
        &mut app,
        &access_token,
        slug,
        url,
        true,
    )
    .await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let existing_destination_id = destination.unwrap().id;

    // check root redirect
    let (status_code, location, _) = helper::root(&mut app, slug).await;
    assert_eq!(StatusCode::PERMANENT_REDIRECT, status_code);
    assert_eq!(Some(url.to_string()), location);

    // some update
    let (status_code, error) =
        helper::maybe_update_destination(&mut app, &access_token, &existing_destination_id, url)
            .await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert_eq!(Some("Permanent URLs can not be updated".to_string()), error);
}
