use axum::http::StatusCode;

use crate::tests::helper;

#[tokio::test]
async fn test_destination_update() {
    let mut app = helper::setup_test_app().await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "";
    let url_one = "https://www.example.com/";
    let url_two = "https://www.dummy.com/";

    // create destination
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url_one).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let existing_destination_id = destination.unwrap().id;

    // check root redirect
    let (status_code, location, _) = helper::root(&mut app, slug).await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(url_one.to_string()), location);

    // update with different url
    let (status_code, _) = helper::maybe_update_destination(
        &mut app,
        &access_token,
        &existing_destination_id,
        url_two,
    )
    .await;
    assert_eq!(StatusCode::OK, status_code);

    // check root redirect
    let (status_code, location, _) = helper::root(&mut app, slug).await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);
    assert_eq!(Some(url_two.to_string()), location);
}
