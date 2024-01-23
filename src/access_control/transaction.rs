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
    /// not committed. If `will_write` is true, uses an `EXCLUSIVE`
    /// transaction (should it take an DEFERRED / IMMEDIATE /
    /// EXCLUSIVE enum?).
    pub fn new(db: &'t mut Db, will_write: bool) -> Result<Self, TransactionError> {
        db.with_connection(|conn, _statements| -> Result<(), TransactionError> {
            match conn.execute(if will_write {"BEGIN EXCLUSIVE TRANSACTION"}
                               else {"BEGIN TRANSACTION"}) {
                Ok(()) => Ok(()),
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
    db: &mut Db, will_write: bool, handler: F
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
        let r: Result<Result<R, E>, TransactionError> = try_result!{
            
            let mut trans = Transaction::new(db, will_write)?;
            let r: Result<R, E> = handler(&mut trans);
            if r.is_ok() {
                trans.commit()?;
            }
            Ok(r)
        };
        macro_rules! retry {
            ( $errkind:expr, $errconstr:expr, $e:ident ) => {{
                let sleeptime = get_sleeptime();
                if sleeptime < max_sleeptime {
                    warn!("transact: on attempt {attempt} got {} error: {:?}",
                          $errkind, $e);
                    attempt += 1;
                    time_guard!("transact sleep");
                    std::thread::sleep(Duration::from_micros(sleeptime as u64));
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
