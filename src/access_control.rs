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
            def_boxed_thiserror};
use self::{db::access_control_transaction,
           types::User,
           trimcheck::InputCheckFailure,
           transaction::TransactionError,
           util::UniqueError,
           sqliteposerror::SQLitePosError};

def_boxed_thiserror!(CheckAccessError, pub enum CheckAccessErrorKind {
    #[error("checking access: password hashing error")]
    HashingError(#[from] HashingError),
    #[error("checking access: input verification failure")]
    InputCheckFailure(#[from] InputCheckFailure),
    #[error("checking access")]
    SQLitePosError(#[from] SQLitePosError),
    #[error("checking access")]
    TransactionError(#[from] TransactionError),
    #[error("checking access")]
    UniqueError(#[from] UniqueError)
});

pub fn check_username_password(
    username: &str, password: &str
) -> Result<Option<User>, CheckAccessError>
{
    let username = trimcheck_username(username)?;
    let password = trimcheck_password(password)?;
    access_control_transaction(|trans| -> Result<Option<User>, CheckAccessError> {
        if let Some(user) = trans.get_user_by_username(username)? {
            if verify_password(password, &user.hashed_pass)? {
                Ok(Some(user))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    })
}
