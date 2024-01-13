use std::cell::RefCell;

use super::sqliteposerror::SQLitePosError;
use super::statements_and_methods::Db;
use super::transaction::{Transaction, TransactionError};

thread_local!{
    static DB: RefCell<Db> =
        RefCell::new(Db::mynew("accounts.db"));
}

pub fn access_control_transaction<F, R, E>(f: F) -> Result<R, E>
where F: FnOnce(&mut Transaction) -> Result<R, E>,
      E: From<SQLitePosError> + From<TransactionError>
{
    DB.with(|b| -> Result<R, E> {
        let mut r = b.borrow_mut();
        let db = &mut *r;
        let mut trans = Transaction::new(db)?;
        let r = f(&mut trans);
        if r.is_ok() {
            trans.commit()?;
        }
        r
    })
}
