use axum::http::StatusCode;

use crate::tests::helper;

#[tokio::test]
async fn test_users() {
    let mut app = helper::setup_test_app().await;

    let access_token = helper::login(&mut app).await;

    let username_one = "someusername";
    let username_two = "someotherusername";
    let password = "somepassword";
    let role = "manager";

    // fetch current user
    let (status_code, current_user) = helper::current_user(&mut app, &access_token).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(current_user.is_some());
    let current_user = current_user.unwrap();

    // fetch users, current user is there
    let (status_code, users) = helper::list_users(&mut app, &access_token).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(users.is_some());
    assert!(users.unwrap().iter().any(|user| user.id == current_user.id));

    // create new user
    let (status_code, user_one, _) =
        helper::maybe_create_user(&mut app, &access_token, username_one, role).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(user_one.is_some());
    let user_one = user_one.unwrap();
    assert_eq!("someusername".to_string(), user_one.username);
    assert!(user_one.password.is_some()); // new password is generated

    // create new user with same username
    let (status_code, _, error) =
        helper::maybe_create_user(&mut app, &access_token, username_one, role).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(error.is_some());
    assert_eq!("User already exists".to_string(), error.unwrap());

    // create new user with password
    let (status_code, user_two, _) = helper::maybe_create_user_with_password(
        &mut app,
        &access_token,
        username_two,
        role,
        Some(password),
    )
    .await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(user_two.is_some());
    let user_two = user_two.unwrap();
    assert_eq!("someotherusername".to_string(), user_two.username);
    assert!(user_two.password.is_none()); // given password is used

    // single user
    let (status_code, user, _) = helper::single_user(&mut app, &access_token, &user_one.id).await;
    assert_eq!(StatusCode::OK, status_code);
    let user = user.unwrap();
    assert_eq!("someusername".to_string(), user.username);
    assert!(user.password.is_none()); // never exposed

    // delete user
    let (status_code, _) = helper::maybe_delete_user(&mut app, &access_token, &user_one.id).await;
    assert_eq!(StatusCode::NO_CONTENT, status_code);

    // single user is no more
    let (status_code, _, error) = helper::single_user(&mut app, &access_token, &user_one.id).await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);
    assert_eq!("User not found".to_string(), error.unwrap());

    // delete user
    let (status_code, error) =
        helper::maybe_delete_user(&mut app, &access_token, &user_one.id).await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);
    assert_eq!("User not found".to_string(), error.unwrap());
}
