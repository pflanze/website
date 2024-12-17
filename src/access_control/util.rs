use std::fmt::Debug;

use sqlite::{State, Bindable, Statement};

use chj_util::notime_guard;

use crate::{def_boxed_thiserror, try_sqlite};

use super::{types::FromStatement, sqliteposerror::SQLitePosError};

def_boxed_thiserror!(UniqueError,  pub enum UniqueErrorKind {
    #[error("getting unique item via {statement_name}: more than one result for arguments {arguments}")]
    MoreThanOne { statement_name: &'static str, arguments: String },
    #[error("getting unique item")]
    SQLitePosError(#[from] SQLitePosError),
});

pub fn get_unique_by<'slf, 's, R, A>(
    statement_name: &'static str, // bundle with sth?
    sth: &'s mut Statement<'slf>,
    arguments: A,
) -> Result<Option<R>, UniqueError>
where R: FromStatement,
      A: Bindable + Debug + Copy // references
{
    notime_guard!(statement_name);
    try_sqlite!(sth.reset());
    try_sqlite!(sth.bind(arguments));
    match try_sqlite!(sth.next()) {
        State::Row => {
            let (r, sth) = try_sqlite!(R::from_statement(sth));
            match try_sqlite!(sth.next()) {
                State::Row => Err(UniqueErrorKind::MoreThanOne {
                    statement_name,
                    arguments: format!("{:?}", arguments)
                }.into()),
                State::Done => Ok(Some(r)),
            }
            },
        State::Done => Ok(None),
    }
}


def_boxed_thiserror!(RequiredUniqueError, pub enum RequiredUniqueErrorKind {
    #[error("retrieving the entry")]
    UniqueError(#[from] UniqueError),
    #[error("{item_type_name} {arguments} not found in the database")]
    MissingError { item_type_name: &'static str, arguments: String },
});
pub fn required_unique<R>(
    item_type_name: &'static str,
    arguments: impl FnOnce() -> String,
    r: Result<Option<R>, UniqueError>
) -> Result<R, RequiredUniqueError>
{
    Ok(r?.ok_or_else(
        || RequiredUniqueErrorKind::MissingError {
            item_type_name,
            arguments: arguments()
        })?)
}

