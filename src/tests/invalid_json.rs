use axum::http::StatusCode;

use crate::tests::helper;

#[tokio::test]
async fn test_invalid_json() {
    let mut app = helper::setup_test_app().await;

    let access_token = helper::login(&mut app).await;

    // missing data
    let body = r"{}";
    let (status_code, _, error) =
        helper::maybe_create_destination_with_raw_body(&mut app, &access_token, body, true).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(error.is_some());
    let error = error.unwrap();
    assert_eq!("Data error".to_string(), error.error);
    assert_eq!(
        Some("Failed to deserialize the JSON body into the target type".to_string()),
        error.description
    );

    // syntax error
    let body = r#"{"}"#;
    let (status_code, _, error) =
        helper::maybe_create_destination_with_raw_body(&mut app, &access_token, body, true).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(error.is_some());
    let error = error.unwrap();
    assert_eq!("JSON syntax error".to_string(), error.error);
    assert_eq!(
        Some("EOF while parsing a string at line 1 column 3".to_string()),
        error.description
    );

    // syntax error
    let body = r#"{"foo":{"bar":}}"#;
    let (status_code, _, error) =
        helper::maybe_create_destination_with_raw_body(&mut app, &access_token, body, true).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(error.is_some());
    let error = error.unwrap();
    assert_eq!("JSON syntax error".to_string(), error.error);
    assert_eq!(
        Some("foo: expected value at line 1 column 15".to_string()),
        error.description
    );

    // missing content type
    let body = r"{}";
    let (status_code, _, error) =
        helper::maybe_create_destination_with_raw_body(&mut app, &access_token, body, false).await;
    assert_eq!(StatusCode::BAD_REQUEST, status_code);
    assert!(error.is_some());
    let error = error.unwrap();
    assert_eq!(
        "Missing `application/json` content type".to_string(),
        error.error
    );
}
