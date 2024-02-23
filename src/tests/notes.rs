use axum::http::StatusCode;

use crate::tests::helper;

#[sqlx::test]
async fn test_notes(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "";
    let url = "https://www.example.com/";

    let content_one = "Ad campaign 27-06";
    let content_two = "Ad campaign 28-06";

    // create destination for notes
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let destination = destination.unwrap();

    // verify empty note list
    let (status_code, notes) = helper::list_notes(&mut app, &access_token, &destination.id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(notes.is_some());
    let notes = notes.unwrap();
    assert_eq!(Vec::<helper::Note>::new(), notes);

    // create note
    let (status_code, note, _) =
        helper::maybe_create_note(&mut app, &access_token, &destination.id, content_one).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(note.is_some());
    let note = note.unwrap();
    assert_eq!(content_one.to_string(), note.content);

    // verify note
    let (status_code, note, _) =
        helper::single_note(&mut app, &access_token, &destination.id, &note.id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(note.is_some());
    let note = note.unwrap();
    assert_eq!(content_one.to_string(), note.content);

    // fetch notes, note is included
    let (status_code, notes) = helper::list_notes(&mut app, &access_token, &destination.id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(notes.is_some());
    assert!(notes.unwrap().iter().any(|note_| note_.id == note.id));

    // update note
    let (status_code, note, _) = helper::maybe_update_note(
        &mut app,
        &access_token,
        &destination.id,
        &note.id,
        content_two,
    )
    .await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(note.is_some());
    let note = note.unwrap();
    assert_eq!(content_two.to_string(), note.content);

    // verify note
    let (status_code, note, _) =
        helper::single_note(&mut app, &access_token, &destination.id, &note.id).await;
    assert_eq!(StatusCode::OK, status_code);
    assert!(note.is_some());
    let note = note.unwrap();
    assert_eq!(content_two.to_string(), note.content);

    // delete note
    let (status_code, _) =
        helper::myabe_delete_note(&mut app, &access_token, &destination.id, &note.id).await;
    assert_eq!(StatusCode::NO_CONTENT, status_code);

    // verify note
    let (status_code, _, error) =
        helper::single_note(&mut app, &access_token, &destination.id, &note.id).await;
    assert_eq!(StatusCode::NOT_FOUND, status_code);
    assert_eq!(Some("Note not found".to_string()), error);
}

#[sqlx::test]
async fn test_note_invalid_id(pool: sqlx::PgPool) {
    let mut app = helper::setup_test_app(pool).await;

    let access_token = helper::login(&mut app).await;

    // setup
    let slug = "";
    let url = "https://www.example.com/";
    let content = "Ad campaign 27-06";
    let invalid_id = "some-id";

    // create destination for notes
    let (status_code, destination, _) =
        helper::maybe_create_destination(&mut app, &access_token, slug, url).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(destination.is_some());
    let destination = destination.unwrap();

    // create note
    let (status_code, note, _) =
        helper::maybe_create_note(&mut app, &access_token, &destination.id, content).await;
    assert_eq!(StatusCode::CREATED, status_code);
    assert!(note.is_some());
    let note = note.unwrap();
    assert_eq!(content.to_string(), note.content);

    // validate uuid
    let (status_code, _, error) =
        helper::single_note_with_str(&mut app, &access_token, &destination.id, invalid_id).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert_eq!(Some("Invalid path parameter".to_string()), error);
}
