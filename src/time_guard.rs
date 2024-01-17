use std::{time::Instant, fmt::Debug};


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

pub struct TimeGuard<S: Debug> {
    pub name: S,
    pub start: Instant
}

impl<S: Debug> Drop for TimeGuard<S> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        eprintln!("{:?}: {:#?}", self.name, elapsed);
    }
}

#[macro_export]
macro_rules! time_guard {
    ($namestr:expr) => {
        let _guard = time_guard::TimeGuard {
            name: $namestr,
            start: std::time::Instant::now()
        };
    }
}

#[macro_export]
macro_rules! notime_guard {
    ($namestr:expr) => {}
}

