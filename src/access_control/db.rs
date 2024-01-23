use std::cell::RefCell;
use std::fmt::Debug;

use super::statements_and_methods::Db;
use super::transaction::{Transaction, transact, TransactError};

thread_local!{
    static DB: RefCell<Db> =
        RefCell::new(Db::mynew("accounts.db"));
}

pub fn access_control_transaction<F, R, E>(f: F) -> Result<R, TransactError<E>>
where F: Fn(&mut Transaction) -> Result<R, E>,
      E: Debug
{
    DB.with(|b| {
        let mut r = b.borrow_mut();
        let db = &mut *r;
        transact(db, f)
    })
}
