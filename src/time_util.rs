use std::time::{SystemTime, Instant};

use chrono::{self, DateTime, NaiveDate, Datelike, TimeZone, NaiveDateTime, Utc};

pub trait LocalYear {
    fn local_year(&self, zone: impl TimeZone) -> i32;
}


impl LocalYear for NaiveDate {
    fn local_year(&self, _zone: impl TimeZone) -> i32 {
        self.year()
    }
}

impl LocalYear for NaiveDateTime {
    fn local_year(&self, _zone: impl TimeZone) -> i32 {
        self.year()
    }
}

impl<AnyTz: TimeZone> LocalYear for DateTime<AnyTz> {
    fn local_year(&self, zone: impl TimeZone) -> i32 {
        let dt = self.with_timezone(&zone);
        dt.naive_local().local_year(zone)
    }
}

impl LocalYear for SystemTime {
    fn local_year(&self, zone: impl TimeZone) -> i32 {
        let dt : DateTime<Utc> = DateTime::from(*self);
        dt.local_year(zone)
    }
}


// ------------------------------------------------------------------

/// Blocking, don't use with async.
pub fn sleep_until(target: Instant) {
    loop {
        let now = Instant::now();
        // let d = target.checked_sub(now);
        if let Some(d) = target.checked_duration_since(now) {
            std::thread::sleep(d);
        } else {
            break
        }
    }
}

