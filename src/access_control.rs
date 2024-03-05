pub mod trimcheck;
pub mod db;
pub mod statements_and_methods;
pub mod types;
pub mod macros;
pub mod transaction;
pub mod util;
pub mod sqliteposerror;

use crate::{access_control::trimcheck::{trimcheck_username, trimcheck_password},
            hash_util::{verify_password, HashingError},
            def_boxed_thiserror, warn};
use self::{db::access_control_transaction,
           types::User,
           trimcheck::InputCheckFailure,
           transaction::{TransactionError, TransactError},
           util::UniqueError,
           sqliteposerror::SQLitePosError, statements_and_methods::sessionid_hash};

def_boxed_thiserror!(CheckAccessError, pub enum CheckAccessErrorKind {
    #[error("checking access: password hashing error")]
    HashingError(#[from] HashingError),
    #[error("checking access: input verification failure")]
    InputCheckFailure(#[from] InputCheckFailure),
    #[error("checking access")]
    SQLitePosError(#[from] SQLitePosError),
    #[error("checking access")]
    UniqueError(#[from] UniqueError),
    #[error("checking access")]
    TransactionError(#[from] TransactionError),
});

pub fn check_username_password(
    username: &str, password: &str
) -> Result<Option<User>, CheckAccessError>
{
    let username = trimcheck_username(username)?;
    let password = trimcheck_password(password)?;
    match access_control_transaction(false, |trans| -> Result<Option<User>, CheckAccessError> {
        if let Some(user) = trans.get_user_by_username(username)? {
            if verify_password(password, &user.hashed_pass)? {
                Ok(Some(user))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }) {
        Ok(v) => Ok(v),
        Err(e) => match e {
            TransactError::TransactionError(e) => Err(e.into()),
            TransactError::HandlerError(e) => Err(e.into())
        }
    }
}


pub fn get_user_from_session_id(
    session_id: &str, hasher: blake3::Hasher
) -> Result<Option<User>, CheckAccessError> {
    let hash = sessionid_hash(hasher, session_id);
    match access_control_transaction(false, |trans| -> Result<Option<User>, CheckAccessError> {
        if let Some(sessiondata) = trans.get_sessiondata_by_sessionid_hash(&hash)? {
            if let Some(id) = sessiondata.user_id {
                // XX why does get_user_by_id not expect UserId ?
                if let Some(user) = trans.get_user_by_id(id.0)? {
                    Ok(Some(user))
                } else {
                    warn!("User deleted while session is still active?");
                    // (Yeah, you definitely need persistent auto-increment!)
                    Ok(None)
                }
            } else {
                warn!("SessionData without user id, not logged in");//
                Ok(None)
            }
        } else {
            // warn!("session expired"); // rather just not logged in yet
            Ok(None)
        }
    }) {
        Ok(v) => Ok(v),
        Err(e) => match e {
            // XX what were the reasons/details here ?
            TransactError::TransactionError(e) => Err(e.into()),
            TransactError::HandlerError(e) => Err(e.into())
        }
    }
}
