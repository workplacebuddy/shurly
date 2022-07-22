use axum::http::StatusCode;

use crate::tests::helper;

#[tokio::test]
async fn test_change_password() {
    let mut app = helper::setup_test_app().await;

    // setup
    let password = "verysecret";
    let new_password = "someotherpassword";
    let wrong_password = "wrongpassword";

    let access_token = helper::login_with_password(&mut app, password).await;

    // check valid token
    let (status_code, _) = helper::list_destinations(&mut app, &access_token).await;
    assert_eq!(StatusCode::OK, status_code);

    // try changing with wrong password
    let (status_code, new_access_token, error) =
        helper::maybe_change_password(&mut app, &access_token, wrong_password, new_password).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(new_access_token.is_none());
    assert_eq!(Some("Invalid password".to_string()), error);

    // try changing with right password
    let (status_code, new_access_token, error) =
        helper::maybe_change_password(&mut app, &access_token, password, new_password).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(new_access_token.is_some());
    assert!(error.is_none());
    let new_access_token = new_access_token.unwrap();

    // check old token
    let (status_code, _) = helper::list_destinations(&mut app, &access_token).await;
    assert_eq!(StatusCode::FORBIDDEN, status_code);

    // check new token
    let (status_code, _) = helper::list_destinations(&mut app, &new_access_token).await;
    assert_eq!(StatusCode::OK, status_code);
}
