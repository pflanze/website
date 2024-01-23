use std::{ops::{Deref, DerefMut}, fmt::Debug, time::Duration};

use crate::{warn, def_boxed_thiserror, try_sqlite};
use super::{statements_and_methods::{DbConnection, ConnectionAndStatements}, sqliteposerror::SQLitePosError};

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
    pub(crate) connection_and_statements: &'t mut ConnectionAndStatements,
    is_committed: bool
}

impl<'t> Transaction<'t> {
    /// Taking mut since there can only be one transaction in
    /// progress. The transaction will be rolled back when dropped and
    /// not committed. If `will_write` is true, uses an `EXCLUSIVE`
    /// transaction (should it take an DEFERRED / IMMEDIATE /
    /// EXCLUSIVE enum?).
    pub fn new(
        connection_and_statements: &'t mut ConnectionAndStatements, will_write: bool
    ) -> Result<Self, TransactionError> {
        connection_and_statements.with_connection(
            |conn, _statements| -> Result<(), TransactionError> {
                match conn.execute(if will_write {"BEGIN EXCLUSIVE TRANSACTION"}
                                   else {"BEGIN TRANSACTION"}) {
                    Ok(()) => Ok(()),
                    Err(e) => Err(TransactionErrorKind::BeginError(e))?
                }
            })?;
        Ok(Self {
            connection_and_statements,
            is_committed: false
        })
    }

    pub fn commit(mut self) -> Result<(), TransactionError> {
        let conn = self.connection_and_statements.connection.as_mut().ok_or_else(
            || TransactionErrorKind::NotConnected)?;
        try_sqlite!(conn.execute("COMMIT TRANSACTION"));
        self.is_committed = true;
        Ok(())
    }
}

// XX do we actually really want to deref?

impl<'t> Deref for Transaction<'t> {
    type Target = ConnectionAndStatements;

    fn deref(&self) -> &Self::Target {
        self.connection_and_statements
    }
}

impl<'t> DerefMut for Transaction<'t> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.connection_and_statements
    }
}

// /do we want to?

impl<'t> Drop for Transaction<'t> {
    fn drop(&mut self) {
        if ! self.is_committed {
            let conn = self.connection_and_statements.connection.as_mut().expect(
                "connected when Transaction exists");
            if let Err(e) = conn.execute("ROLLBACK TRANSACTION") {
                warn!("drop Transaction: ROLLBACK gave error: {e:?}");
            }
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

/// Run a transaction on `db`. The `handler` gets the transaction
/// handle (on which it can find the methods to interact with the db;
/// this is a hack and to be replaced with something scalable). If the
/// handler returns a database error that has a chance of succeeding
/// via retry (SQLite issues SQLITE_BUSY (code 5) errors when multiple
/// threads are running a writing transaction at the same time), the
/// handler is re-run in a new transaction while sleeping between
/// retries with exponential backoff, until the handler succeeds or
/// about 2-4 seconds have passed, at which point the error is
/// returned. If `will_write` is true, locks a mutex first, to avoid
/// needless retries (hopefully lowering both the latency and CPU
/// usage) and starts an `EXCLUSIVE` transaction. The handler can
/// still write to the db even if `will_write` is false, but in that
/// case retries will definitely happen when concurrent accesses
/// happen.
pub fn transact<F, R, E>(
    dbconnection: &mut DbConnection, will_write: bool, handler: F
) -> Result<R, TransactError<E>>
where F: Fn(&mut Transaction) -> Result<R, E>,
      E: Debug
{
    // Sleep with exponential backoff (XX: should perhaps use some randomization)
    let mut get_sleeptime = {
        let mut sleeptime: u32 = 500; // microseconds
        move || {
            let old_sleeptime = sleeptime;
            sleeptime = old_sleeptime * 5 / 4;
            old_sleeptime
        }
    };
    // 1_000_000 with `* 5 / 4` leads to 36 attempts accumulating 6.1
    // seconds.
    let max_sleeptime: u32 = 1_000_000; // microseconds
    let mut attempt = 1;

    loop {
        let run_trans = |cs| {
            let mut trans = Transaction::new(cs, will_write)?;
            let r: Result<R, E> = handler(&mut trans);
            if r.is_ok() {
                trans.commit()?;
            }
            Ok(r)
        };
        let r: Result<Result<R, E>, TransactionError> =
            if will_write {
                let _guard = dbconnection.db.write_transaction_mutex.lock();
                run_trans(&mut dbconnection.connection_and_statements)
            } else {
                run_trans(&mut dbconnection.connection_and_statements)
            };
        macro_rules! retry {
            ( $errkind:expr, $errconstr:expr, $e:ident ) => {{
                let sleeptime = get_sleeptime();
                if sleeptime < max_sleeptime {
                    attempt += 1;
                    std::thread::sleep(Duration::from_micros(sleeptime as u64));
                } else {
                    warn!("transact: ran out of retries");
                    return Err($errconstr($e))
                }
            }}
        }
        match r {
            Ok(r) => match r {
                Ok(v) => {
                    if attempt > 1 {
                        warn!("transact: succeeded on attempt {attempt}");
                    }
                    return Ok(v)
                }
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
