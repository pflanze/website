use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::Arc;

use lazy_static::lazy_static;

use super::statements_and_methods::{Db, DbConnection};
use super::transaction::{Transaction, transact, TransactError};

lazy_static!{
    static ref DB: Arc<Db> = Arc::new(Db::new("accounts.db"));
}

thread_local!{
    static DBCONNECTION: RefCell<DbConnection> =
        RefCell::new(DbConnection::mynew(DB.clone()));
}

pub fn access_control_transaction<F, R, E>(
    will_write: bool, f: F
) -> Result<R, TransactError<E>>
where F: Fn(&mut Transaction) -> Result<R, E>,
      E: Debug
{
    DBCONNECTION.with(|b| {
        let mut r = b.borrow_mut();
        let db = &mut *r;
        transact(db, will_write, f)
    })
}
