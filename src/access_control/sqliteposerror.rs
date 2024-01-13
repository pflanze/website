use std::fmt::Display;

use crate::def_boxed_thiserror;


def_boxed_thiserror!(SQLitePosError, pub struct SQLitePosErrorInner {
    pub error: sqlite::Error,
    pub file: &'static str,
    pub line: u32
});

impl Display for SQLitePosErrorInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("SQLite error: {} at {:?} line {}",
                                 self.error,
                                 self.file,
                                 self.line))
    }
}

#[macro_export]
macro_rules! try_sqlite {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => {
                let e: crate::access_control::sqliteposerror::SQLitePosError =
                    crate::access_control::sqliteposerror::SQLitePosErrorInner {
                        error: e, file: file!(), line: line!()
                    }.into();
                Err(e)?
            }
        }
    }
}

