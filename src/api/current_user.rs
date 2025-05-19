//! Current user service
//!
//! Get the current user from the request based on the Authorization header

use std::ops::Deref;
use std::sync::Arc;

use axum::Extension;
use axum::RequestPartsExt;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::EncodingKey;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::api::Error;
use crate::database::Database;
use crate::users::User;

/// The keys used for encoding/decoding JWT tokens
#[derive(Clone)]
pub struct JwtKeys {
    /// The encoding key
    encoding: EncodingKey,

    /// The decoding key
    decoding: DecodingKey,
}

impl JwtKeys {
    /// Create new encoding/decoding keys, derived from a secret
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

/// The JWT claims to identifies a user
#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    /// The user ID
    sub: Uuid,

    /// In how many seconds does the token expire
    exp: i64,

    /// A sessions ID, used to expire/invalidate tokens before the expiration date
    jti: Uuid,
}

/// Token information served to the user
#[derive(Debug, Serialize)]
pub struct Token {
    /// Type of the token: Bearer
    #[allow(clippy::struct_field_names)] // `type` is a reserved keyword
    token_type: String,

    /// In how many seconds does the token expire
    expires_in: i64,

    /// The access token to provide to follow up requests in the Authorization header
    #[allow(clippy::struct_field_names)] // `access_token` is the name of the field
    access_token: String,
}

impl Token {
    /// Create a new token response
    fn new(access_token: String, expires_in: i64) -> Self {
        Self {
            token_type: "Bearer".to_string(),
            expires_in,
            access_token,
        }
    }
}

/// Current user service
#[derive(Clone)]
pub struct CurrentUser {
    /// The actual user
    user: Arc<User>,
}

impl CurrentUser {
    /// Create the current user from a user
    fn new(user: User) -> Self {
        Self {
            user: Arc::new(user),
        }
    }
}

impl Deref for CurrentUser {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

/// Generate a token for the outside world for a given user
pub fn generate_token(jwt_keys: &JwtKeys, user: &User) -> Result<Token, Error> {
    use jsonwebtoken::Header;
    use jsonwebtoken::encode;

    let expires_in = 3600; // valid for an hour
    let claims = Claims {
        sub: user.id,
        exp: chrono::Utc::now().timestamp() + expires_in,
        jti: user.session_id,
    };

    let access_token = encode(&Header::default(), &claims, &jwt_keys.encoding)
        .map_err(Error::internal_server_error)?;

    Ok(Token::new(access_token, expires_in))
}

impl<B> FromRequestParts<B> for CurrentUser
where
    B: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &B) -> Result<Self, Self::Rejection> {
        use jsonwebtoken::Validation;
        use jsonwebtoken::decode;

        // Extract the token from the authorization header
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| Error::forbidden("Missing API token"))?;

        let Extension(jwt_keys) = parts
            .extract::<Extension<JwtKeys>>()
            .await
            .map_err(|_| Error::internal_server_error("Could not get JWT keys"))?;

        let Extension(database) = parts
            .extract::<Extension<Database>>()
            .await
            .map_err(|_| Error::internal_server_error("Could not get a database pool"))?;

        let validation = Validation::default();

        // Decode the user data
        let token_data = decode::<Claims>(bearer.token(), &jwt_keys.decoding, &validation)
            .map_err(|err| Error::forbidden(format!("Invalid token: {err}")))?;

        let claims = token_data.claims;

        let id = claims.sub;

        let user = database
            .find_single_user_by_id(&id)
            .await
            .map_err(|_| Error::forbidden("Could not find user"))?;

        if let Some(user) = user {
            // mechanism to invalidate JWT tokens
            if claims.jti != user.session_id {
                return Err(Error::forbidden("Token expired"));
            }

            Ok(CurrentUser::new(user))
        } else {
            Err(Error::forbidden("Could not find user"))
        }
    }
}
