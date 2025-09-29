use axum::http::StatusCode;

use crate::tests::helper;

#[sqlx::test]
async fn test_aliases(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "";
    let url = "https://www.example.com/";

    let alias_slug = "example";

    // create destination for aliases
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let destination = destination.unwrap();

    let (status_code, _, _) = helper::root(&mut app, slug).await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);

    let (status_code, _, _) = helper::root(&mut app, alias_slug).await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);

    // verify empty alias list
    let (status_code, aliases) =
        helper::list_aliases(&mut app, &access_token, &destination.id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(aliases.is_some());
    let notes = aliases.unwrap();
    assert_eq!(Vec::<helper::Alias>::new(), notes);

    // create alias
    let (status_code, alias, _) =
        helper::maybe_create_alias(&mut app, &access_token, &destination.id, alias_slug).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(alias.is_some());
    let alias = alias.unwrap();
    assert_eq!(alias_slug.to_string(), alias.slug);

    // verify alias
    let (status_code, alias, _) =
        helper::single_alias(&mut app, &access_token, &destination.id, &alias.id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(alias.is_some());
    let alias = alias.unwrap();
    assert_eq!(alias_slug.to_string(), alias.slug);

    // fetch aliases, alias is included
    let (status_code, aliases) =
        helper::list_aliases(&mut app, &access_token, &destination.id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(aliases.is_some());
    assert!(aliases.unwrap().iter().any(|alias_| alias_.id == alias.id));

    let (status_code, _, _) = helper::root(&mut app, alias_slug).await;
    assert_eq!(StatusCode::TEMPORARY_REDIRECT, status_code);

    // delete alias
    let (status_code, _) =
        helper::myabe_delete_alias(&mut app, &access_token, &destination.id, &alias.id).await;
    assert_eq!(StatusCode::NO_CONTENT, status_code);

    // verify alias
    let (status_code, _, error) =
        helper::single_alias(&mut app, &access_token, &destination.id, &alias.id).await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);
    assert_eq!(Some("Alias not found".to_string()), error);

    let (status_code, _, _) = helper::root(&mut app, alias_slug).await;
    assert_eq!(StatusCode::GONE, status_code);
}

#[sqlx::test]
async fn test_alias_invalid_id(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "";
    let url = "https://www.example.com/";
    let alias_slug = "example";
    let invalid_id = "some-id";

    // create destination for aliases
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let destination = destination.unwrap();

    // create alias
    let (status_code, alias, _) =
        helper::maybe_create_alias(&mut app, &access_token, &destination.id, alias_slug).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(alias.is_some());
    let alias = alias.unwrap();
    assert_eq!(alias_slug.to_string(), alias.slug);

    // validate uuid
    let (status_code, _, error) =
        helper::single_alias_with_str(&mut app, &access_token, &destination.id, invalid_id).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert_eq!(Some("Invalid path parameter".to_string()), error);
}
