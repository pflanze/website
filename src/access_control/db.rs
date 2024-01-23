use std::cell::RefCell;
use std::fmt::Debug;

use super::statements_and_methods::DbConnection;
use super::transaction::{Transaction, transact, TransactError};

thread_local!{
    static DB: RefCell<DbConnection> =
        RefCell::new(DbConnection::mynew("accounts.db"));
}

pub fn access_control_transaction<F, R, E>(
    will_write: bool, f: F
) -> Result<R, TransactError<E>>
where F: Fn(&mut Transaction) -> Result<R, E>,
      E: Debug
{
    DB.with(|b| {
        let mut r = b.borrow_mut();
        let db = &mut *r;
        transact(db, will_write, f)
    })
}
