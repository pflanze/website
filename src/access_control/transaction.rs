use std::ops::{Deref, DerefMut};

use crate::{warn, def_boxed_thiserror, try_sqlite};
use super::{statements_and_methods::Db, sqliteposerror::SQLitePosError};

def_boxed_thiserror!(TransactionError, pub enum TransactionErrorKind {
    #[error("Db is not connected")]
    NotConnected,
    #[error("sqlite error: {0}")]
    SQLitePosError(#[from] SQLitePosError),
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
            try_sqlite!(conn.execute("BEGIN TRANSACTION"));
            Ok(())
        })?;
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
        }
    }
}
