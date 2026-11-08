//! Umm al-Qura calendar, using the same month table as the phone's `java.time.chrono.HijrahDate`.
//! `data/hijri_table.txt` is dumped from the JVM by tools/golden, so the two calendars cannot disagree.

use jiff::civil::Date;
use std::sync::OnceLock;

struct Table {
    first_year: i32,
    first_epoch_day: i64,
    /// Twelve month lengths per year, oldest year first.
    months: Vec<u8>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut lines = include_str!("../data/hijri_table.txt").lines();
        let mut value = || -> i64 { lines.next().unwrap().split('=').nth(1).unwrap().parse().unwrap() };
        let first_year = value() as i32;
        let _last_year = value();
        let first_epoch_day = value();
        let months = lines.flat_map(|l| l.split(',')).map(|n| n.parse().unwrap()).collect();
        Table { first_year, first_epoch_day, months }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HijriDate {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    pub day: u8,
}

fn epoch_day(date: Date) -> i64 {
    date.since((jiff::Unit::Day, Date::constant(1970, 1, 1))).expect("date in range").get_days() as i64
}

/// The Hijri date of a Gregorian day, or None outside the table (1300 to 1600 AH).
pub fn from_gregorian(date: Date) -> Option<HijriDate> {
    let t = table();
    let mut remaining = epoch_day(date) - t.first_epoch_day;
    if remaining < 0 {
        return None;
    }
    for (i, &len) in t.months.iter().enumerate() {
        if remaining < len as i64 {
            return Some(HijriDate { year: t.first_year + (i / 12) as i32, month: (i % 12) as u8 + 1, day: remaining as u8 + 1 });
        }
        remaining -= len as i64;
    }
    None
}
