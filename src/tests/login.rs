use crate::tests::helper;

#[tokio::test]
async fn test_login() {
    let mut app = helper::setup_test_app().await;

    let access_token = helper::login(&mut app).await;
    assert!(access_token.len() > 10);
}
