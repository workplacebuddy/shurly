use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use axum::async_trait;
use axum::extract::FromRequest;
use axum::extract::RequestParts;
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::Extension;
use axum::TypedHeader;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::EncodingKey;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::api::Error;
use crate::storage::Storage;
use crate::users::User;

#[derive(Clone)]
pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtKeys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    sub: Uuid,
    exp: i64,
    jti: Uuid,
}

#[derive(Debug, Serialize)]
pub struct Token {
    token_type: String,
    expires_in: i64,
    access_token: String,
}

impl Token {
    fn new(access_token: String, expires_in: i64) -> Self {
        Self {
            token_type: "Bearer".to_string(),
            expires_in,
            access_token,
        }
    }
}

#[derive(Clone)]
pub struct CurrentUser<S: Storage> {
    user: Arc<User>,
    storage: PhantomData<S>,
}

impl<S: Storage> CurrentUser<S> {
    fn new(user: User) -> Self {
        Self {
            user: Arc::new(user),
            storage: PhantomData,
        }
    }
}

impl<S: Storage> Deref for CurrentUser<S> {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

pub fn generate_token(jwt_keys: &JwtKeys, user: &User) -> Result<Token, Error> {
    use jsonwebtoken::encode;
    use jsonwebtoken::Header;

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

#[async_trait]
impl<B, S> FromRequest<B> for CurrentUser<S>
where
    B: Send,
    S: Storage,
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        use jsonwebtoken::decode;
        use jsonwebtoken::Validation;

        // Extract the token from the authorization header
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request(req)
                .await
                .map_err(|_| Error::forbidden("Missing API token"))?;

        let Extension(jwt_keys) = req
            .extract::<Extension<JwtKeys>>()
            .await
            .map_err(|_| Error::internal_server_error("Could not get JWT keys"))?;

        let Extension(storage) = req
            .extract::<Extension<S>>()
            .await
            .map_err(|_| Error::internal_server_error("Could not get a storage pool"))?;

        let validation = Validation::default();

        // Decode the user data
        let token_data = decode::<Claims>(bearer.token(), &jwt_keys.decoding, &validation)
            .map_err(|err| Error::forbidden(format!("Invalid token: {err}")))?;

        let claims = token_data.claims;

        let id = claims.sub;

        let user = storage
            .find_single_user_by_id(&id)
            .await
            .map_err(|_| Error::forbidden("Could not find user"))?;

        if let Some(user) = user {
            if claims.jti != user.session_id {
                return Err(Error::forbidden("Token expired"));
            }

            Ok(CurrentUser::new(user))
        } else {
            Err(Error::forbidden("Could not find user"))
        }
    }
}
