use crate::tests::helper;

#[sqlx::test]
async fn test_login(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;
    assert!(access_token.len() > 10);
}
