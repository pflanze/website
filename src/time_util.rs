//! Utilities for handling time (interfacing with `std::time` and `chrono`).

use std::time::{SystemTime, Instant, Duration};

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

pub fn now_unixtime() -> i64 {
    let now = SystemTime::now();
    let now_unixtime: u64 = now.duration_since(SystemTime::UNIX_EPOCH)
        .expect("no overflows, we are after epoch").as_secs();
    now_unixtime as i64
}


// ------------------------------------------------------------------

/// Display in a manner that is nice to read for a human, even if
/// approximate, not precise.
pub trait ApproximateDisplay {
    fn to_approximate_display_string(self) -> String;
}

impl ApproximateDisplay for Duration {
    fn to_approximate_display_string(self) -> String {
        let s = self.as_secs();
        if s < 60 {
            format!("{s} second{}",
                    if s == 1 { "" } else { "s" })
        } else if s < 3600 {
            let m = s / 60;
            format!("{m} minute{}",
                    if m == 1 { "" } else { "s" })
        } else if s < 3600*24 {
            let h = s / 3600;
            format!("{h} hour{}",
                    if h == 1 { "" } else { "s" })
        } else if s < 3600*24*30 { // XX precision?
            let d = s / (3600*24);
            format!("{d} day{}",
                    if d == 1 { "" } else { "s" })
        } else if s < 3600*24*365 {
            let m = s / (3600*24*30); // XX precision?
            format!("{m} month{}",
                    if m == 1 { "" } else { "s" })
        } else {
            let y = s / (3600*24*365);
            format!("{y} year{}",
                    if y == 1 { "" } else { "s" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_rough() {
        assert_eq!(Duration::from_secs(0).to_approximate_display_string(),
                   "0 seconds");
        assert_eq!(Duration::from_secs(59).to_approximate_display_string(),
                   "59 seconds");
        assert_eq!(Duration::from_secs(60).to_approximate_display_string(),
                   "1 minute");
        assert_eq!(Duration::from_secs(60*30).to_approximate_display_string(),
                   "30 minutes");
        assert_eq!(Duration::from_secs(60*60).to_approximate_display_string(),
                   "1 hour");
        assert_eq!(Duration::from_secs(60*60*24*3-1).to_approximate_display_string(),
                   "2 days");
        assert_eq!(Duration::from_secs(60*60*24*30-1).to_approximate_display_string(),
                   "29 days");
        assert_eq!(Duration::from_secs(60*60*24*30).to_approximate_display_string(),
                   "1 month");
        // Sun Jan 15 01:00:00 CET 2023 - Mon 15 Jan 01:00:00 CET 2024
        assert_eq!(
            Duration::from_secs(1705276800 - 1673740800).to_approximate_display_string(),
            "1 year");
        assert_eq!(
            Duration::from_secs(1705276800 - 1673740800 - 1).to_approximate_display_string(),
            "12 months");
    }
}
