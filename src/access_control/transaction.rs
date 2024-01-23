use std::{ops::{Deref, DerefMut}, fmt::Debug, time::Duration};

use crate::{warn, def_boxed_thiserror, try_sqlite, try_result};
use super::{statements_and_methods::Db, sqliteposerror::SQLitePosError};

def_boxed_thiserror!(TransactionError, pub enum TransactionErrorKind {
    #[error("sqlite Db is not connected")]
    NotConnected,
    #[error("sqlite initialisation error: {0}")]
    InitError(#[from] SQLitePosError),
    #[error("sqlite error on transaction begin: {0}")]
    BeginError(sqlite::Error),
    #[error("sqlite error on transaction commit: {0}")]
    CommitError(sqlite::Error),
});

pub struct Transaction<'t> {
    db: &'t mut Db,
    is_committed: bool
}

impl<'t> Transaction<'t> {
    /// Taking mut since there can only be one transaction in
    /// progress. The transaction will be rolled back when dropped and
    /// not committed.
    pub fn new(db: &'t mut Db) -> Result<Self, TransactionError> {
        db.with_connection(|conn, _statements| -> Result<(), TransactionError> {
            match conn.execute("BEGIN TRANSACTION") {
                Ok(_) => Ok(()),
                Err(e) => Err(TransactionErrorKind::BeginError(e))?
            }
        })?;
        warn!("begun transaction");
        Ok(Self {
            db,
            is_committed: false
        })
    }

    pub fn commit(mut self) -> Result<(), TransactionError> {
        let conn = self.db.connection.as_mut().ok_or_else(
            || TransactionErrorKind::NotConnected)?;
        try_sqlite!(conn.execute("COMMIT TRANSACTION"));
        self.is_committed = true;
        warn!("committed transaction");
        Ok(())
    }
}

// XX do we actually really want to deref?

impl<'t> Deref for Transaction<'t> {
    type Target = Db;

    fn deref(&self) -> &Self::Target {
        self.db
    }
}

impl<'t> DerefMut for Transaction<'t> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.db
    }
}

// /do we want to?

impl<'t> Drop for Transaction<'t> {
    fn drop(&mut self) {
        if ! self.is_committed {
            let conn = self.db.connection.as_mut().expect(
                "connected when Transaction exists");
            if let Err(e) = conn.execute("ROLLBACK TRANSACTION") {
                warn!("drop Transaction: ROLLBACK gave error: {e:?}");
            }
            warn!("rolled back transaction");
        }
    }
}


// Can't take type arguments with def_boxed_thiserror, thus use an
// unboxed representation.
#[derive(thiserror::Error, Debug)]
pub enum TransactError<E: Debug> {
    #[error("transaction error: {0}")]
    TransactionError(TransactionError),
    #[error("error in transaction handler: {0}")]
    HandlerError(E),
}

pub fn transact<F, R, E>(db: &mut Db, f: F) -> Result<R, TransactError<E>>
where F: Fn(&mut Transaction) -> Result<R, E>,
      E: Debug
{
    let mut sleeptime = 500; // microseconds
    let mut attempt = 1;
    let last_attempt = 12; // ~2 seconds total
    loop {
        let r: Result<Result<R, E>, TransactionError> = try_result!{
            let mut trans = Transaction::new(db)?;
            let r: Result<R, E> = f(&mut trans);
            if r.is_ok() {
                trans.commit()?;
            }
            Ok(r)
        };
        macro_rules! retry {
            ( $errkind:expr, $errconstr:expr, $e:ident ) => {{
                if attempt < last_attempt {
                    warn!("transact: on attempt {attempt} got {} error: {:?}",
                          $errkind, $e);
                    attempt += 1;
                    time_guard!("transact sleep");
                    std::thread::sleep(Duration::from_micros(sleeptime));
                    sleeptime *= 2;
                } else {
                    return Err($errconstr($e))
                }
            }}
        }
        match r {
            Ok(r) => match r {
                Ok(v) => return Ok(v),
                Err(e) => 
                    // Do we *have* to retry these, too? Yes.
                    retry!("handler", TransactError::HandlerError, e),
            }
            Err(e) => {
                macro_rules! immediate {
                    () => {
                        return Err(TransactError::TransactionError(e))
                    }
                }
                    
                match &*e {
                    TransactionErrorKind::NotConnected => immediate!(),
                    TransactionErrorKind::InitError(_) => immediate!(),
                    TransactionErrorKind::BeginError(_) =>
                        retry!("transaction", TransactError::TransactionError, e),
                    TransactionErrorKind::CommitError(_) =>
                        retry!("transaction", TransactError::TransactionError, e),
                }
            }
        }
    }
}
