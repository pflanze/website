//! Multilingual date formatting.

use std::time::SystemTime;

use chrono::{TimeZone, DateTime, Timelike, Datelike};
use chrono_tz::Tz;

use crate::lang_en_de::Lang;

// How many times will I write these up?

pub const fn months_short(lang: Lang) -> &'static [&'static str; 12] {
    match lang {
        Lang::En => &["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        Lang::De => &["Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
    }
}

pub const fn wdays_short(lang: Lang) -> &'static [&'static str; 7] {
    match lang {
        Lang::En => &["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
        Lang::De => &["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
    }
}

/// In the style of the HTTP date format (including named offset
/// information, always 24-hour format).
pub fn date_format_httplike(t: SystemTime, zone: Tz, lang: Lang) -> String {
    let now_unixtime: u64 = t.duration_since(SystemTime::UNIX_EPOCH)
        .expect("no overflow for sensible times")
        .as_secs();
    let dt: DateTime<Tz> = match zone.timestamp_opt(now_unixtime as i64, 0) {
        chrono::LocalResult::None =>
            panic!("Error converting to DateTime, is SystemTime in invalid range?: {t:?}"),
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt1, _dt2) => {
            // This happens on leap seconds, right? Just ignore that
            // detail :/ (There is nothing better we can get out of
            // `t`, right? Oh, timestamp_opt docs say only None or
            // Single are ever returned (and the code matches the
            // docs). Huh, when leap seconds actually *are* ambiguous,
            // no??)
            dt1
        }
    };
    let wday = wdays_short(lang)[dt.weekday().num_days_from_monday() as usize];
    let day = dt.day();
    let month = months_short(lang)[dt.month0() as usize];
    let year = dt.year();
    let h = dt.hour();
    let m = dt.minute();
    let s = dt.second();
    // zone.name() is just e.g. "Europe/Zurich", of course by
    // necessity since zone doesn't have access to the date/time and
    // hence can't know if that is to be represented as dst.
    let offset = dt.offset();
    let zoneshort = offset.to_string();
    match lang {
        Lang::En => format!("{wday}, {day} {month} {year} {h:02}:{m:02}:{s:02} {zoneshort}"),
        Lang::De => format!("{wday}, {day}. {month} {year} {h:02}:{m:02}:{s:02} {zoneshort}"),
    }
}


