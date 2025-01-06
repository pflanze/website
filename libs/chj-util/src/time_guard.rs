//! # Tools for performance debugging.

//! `time!` is currently always enabled. `time_guard!` is only enabled
//! if the `TIME_GUARD` env var is set to a truthy value or
//! `enabled_set(true)` was called in the thread.

//! `time!` and `time_guard!` can also be statically disabled
//! (compiled out completely) by prefixing their names with `no`.

use std::{time::Instant, fmt::Debug, cell::Cell, os::unix::prelude::OsStrExt};

fn time_guard_env_get() -> bool {
    match std::env::var_os("TIME_GUARD") {
        Some(v) => match v.as_bytes() {
            b"0" | b"" | b"off" | b"false" | b"no" => false,
            _ => true
        }
        None => false
    }
}

thread_local!{
    pub static ENABLED: Cell<bool> = Cell::new(time_guard_env_get());
}

/// Enable `time_guard!`.
pub fn enabled_set(on: bool) {
    ENABLED.with(|cell| cell.set(on))
}

pub fn enabled() -> bool {
    ENABLED.with(|old| old.get())
}


// XX also enable/disable?
#[macro_export]
macro_rules! time {
    ($name:expr; $($code:tt)*) => {{
        let msg = format!("time {}", $name);
        let now = std::time::Instant::now();
        let r = {
            $($code)*
        };
        let elapsed = now.elapsed();
        eprintln!("{msg}: {elapsed:?} at {:?} line {}", file!(), line!());
        r
    }}
}

#[macro_export]
macro_rules! notime {
    ($name:expr; $($code:tt)*) => {{
        $($code)*
    }}
}



// Reminiscent of dt.rs (DtGuard)

pub enum TimeGuard<S: Debug> {
    Disabled,
    Enabled {
         name: S,
        start: Instant
    },
}

impl<S: Debug> Drop for TimeGuard<S> {
    fn drop(&mut self) {
        match self {
            TimeGuard::Disabled => (),
            TimeGuard::Enabled { name, start } => {
                let elapsed = start.elapsed();
                eprintln!("{:?}: {:#?}", name, elapsed);
            },
        }
    }
}

#[macro_export]
macro_rules! time_guard {
    ($namestr:expr) => {
        let _guard = if $crate::time_guard::enabled() {
            $crate::time_guard::TimeGuard::Enabled {
                name: $namestr,
                start: std::time::Instant::now()
            }
        } else {
            $crate::time_guard::TimeGuard::Disabled
        };
    }
}

#[macro_export]
macro_rules! notime_guard {
    ($namestr:expr) => {}
}

