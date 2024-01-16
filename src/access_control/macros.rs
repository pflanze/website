
// sqlite::Error doesn't implement Clone, so we do
pub fn clone_sqlite_error(e: &sqlite::Error) -> sqlite::Error {
    sqlite::Error { code: e.code.clone(), message: e.message.clone() }
}

#[macro_export]
macro_rules! get_statement {
    { $connection:expr, $db:expr, $select_field:ident, $sql:expr } => {
        {
            let rsth =
                if let Some(rsth) = &mut $db.$select_field {
                    rsth
                } else {
                    let connectionp: *const Connection = $connection;
                    // Safe because connection is pinned, in a private
                    // field, and living as long as the reference
                    // we're making here (XXX: we rely on lifetime
                    // inference, which relies on the surrounding
                    // function declaration, which means the macro
                    // doesn't properly encapsulate it. TODO fix.)
                    let connection: &Connection = unsafe { &*connectionp };
                    let rsth = connection.prepare($sql);
                    $db.$select_field = Some(rsth);
                    let nam = stringify!($select_field);
                    warn_thread!("initialized select field {:?}", nam);
                    $db.$select_field.as_mut().unwrap()
                };
            match rsth.as_mut() {
                Ok(sth) => sth,
                Err(eref) => crate::try_sqlite!(Err(
                    crate::access_control::macros::clone_sqlite_error(eref)))
            }
        }
    }
}

#[macro_export]
macro_rules! defn_with_statement {
    { $with_statement:ident, $statementvar:ident, $statementstr:expr } => {
        impl Db {
            #[inline]
            pub fn $with_statement<'s, F, R, E>(
                &'s mut self, f: F
            ) -> Result<R, E>
            where F: FnOnce(&'s mut Statement) -> Result<R, E>,
            // with_connection injects this error:
                  E: From<crate::access_control::sqliteposerror::SQLitePosError>
            {
                self.with_connection(|c, s| -> Result<R, E> {
                    let sth: *mut Statement = get_statement!(
                        c, s, $statementvar, $statementstr);
                    f(unsafe { &mut *sth })
                })
            }
        }
    }
}

