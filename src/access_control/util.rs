use std::fmt::Debug;

use sqlite::{State, Bindable, Statement};

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
