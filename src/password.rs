//! Password utilities

use argon2::Argon2;
use argon2::password_hash::PasswordHash;
use argon2::password_hash::PasswordHasher;
use argon2::password_hash::PasswordVerifier;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;

/// Generate a new password
pub fn generate() -> String {
    SaltString::generate(&mut OsRng).to_string()
}

/// Hash a given password
pub fn hash(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);

    let argon2 = Argon2::default();

    let hashed_password = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Valid hashed password");

    hashed_password.to_string()
}

/// Verify a given password against a given hash
pub fn verify(hashed_password: &str, password: &str) -> bool {
    let parsed_hash = PasswordHash::new(hashed_password).expect("Valid parsed hash");

    let argon2 = Argon2::default();

    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}
