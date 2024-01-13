use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};

use crate::def_boxed_thiserror;

def_boxed_thiserror!(HashingError, pub enum HashingErrorKind {
    #[error("argon2 hashing error: {0}")]
    Argon2(argon2::password_hash::Error),
});
// Somehow StdError not implemented shows up then trying to use
// #[from] above, thus manually:
impl From<argon2::password_hash::Error> for HashingErrorKind {
    fn from(e: argon2::password_hash::Error) -> Self {
        HashingErrorKind::Argon2(e)
    }
}

pub fn create_password_hash(password: &str) -> Result<String, HashingError> {
    let salt = SaltString::generate(&mut OsRng);
    // Argon2 with default params (Argon2id v19)
    let argon2 = Argon2::default();
    // Hash password to PHC string ($argon2id$v=19$...)
    let pw = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(pw.to_string())
}
                     
pub fn verify_password(password: &str,
                       existing_hash: &str) -> Result<bool, HashingError> {
    let parsed_hash = PasswordHash::new(&existing_hash)?;
    match Argon2::default().verify_password(password.as_bytes(),
                                            &parsed_hash)
    {
        Ok(()) => Ok(true),
        Err(e) => match e {
            argon2::password_hash::Error::Password => Ok(false),
            _ => Err(e.into())
        }
    }
}
