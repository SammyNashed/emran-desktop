//! Compares the port with data dumped from the Android app's real Kotlin file (see tools/golden).

use emran_core::cities::CITIES;
use emran_core::hijri::from_gregorian;
use emran_core::prayer::*;
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::Timestamp;

fn fmt(t: DateTime) -> String {
    t.strftime("%Y-%m-%dT%H:%M:%S").to_string()
}

fn row_options(c: &[&str]) -> (Date, Options) {
    let adj: Vec<i32> = c[7].split('|').map(|n| n.parse().unwrap()).collect();
    let custom = (!c[8].is_empty()).then(|| {
        let p: Vec<f64> = c[8].split('|').map(|n| n.parse().unwrap()).collect();
        CustomParameters { fajr_angle: p[0], isha_angle: p[1], isha_interval: p[2] as i32, maghrib_angle: p[3] }
    });
    let options = Options {
        latitude: c[1].parse().unwrap(),
        longitude: c[2].parse().unwrap(),
        zone: TimeZone::get(c[3]).unwrap(),
        method: Method::from_key(c[4]).unwrap(),
        school: AsrSchool::from_key(c[5]).unwrap(),
        high_latitude_rule: HighLatitudeRule::from_key(c[6]).unwrap(),
        adjustments: Adjustments { fajr: adj[0], sunrise: adj[1], dhuhr: adj[2], asr: adj[3], maghrib: adj[4], isha: adj[5] },
        custom,
        use_ramadan_isha_interval: c[9] == "true",
    };
    (c[0].parse().unwrap(), options)
}

/// Morocco moved to UTC+0 on 2026-09-20 (tzdata 2026c and later). The JDK that dumped the golden file
/// has tzdata 2026b, so it is an hour ahead on these dates only. Nothing else is excused.
fn expected_value(row: &[&str], column: usize, value: &str) -> String {
    let stale_jvm_zone = row[3] == "Africa/Casablanca" && (row[0] == "2026-11-01" || row[0] == "2026-11-08");
    if !stale_jvm_zone || column > 5 {
        return value.to_string();
    }
    let t: DateTime = value.parse().unwrap();
    fmt(t.checked_sub(jiff::ToSpan::hours(1)).unwrap())
}

#[test]
fn every_row_matches_the_android_app_to_the_second() {
    let data = include_str!("data/golden_times.csv");
    let (mut rows, mut failures) = (0, Vec::new());
    for line in data.lines().skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        let (date, options) = row_options(&c);
        let t = calculate_times(date, &options);
        let got = [
            fmt(t.fajr), fmt(t.sunrise), fmt(t.dhuhr), fmt(t.asr), fmt(t.maghrib), fmt(t.isha),
            t.has_true_sunrise_and_sunset.to_string(), t.twilight_is_astronomical.to_string(),
        ];
        rows += 1;
        let expected: Vec<String> = c[10..18].iter().enumerate().map(|(i, v)| expected_value(&c, i, v)).collect();
        if got.as_slice() != expected.as_slice() {
            failures.push(format!("{line}\n  got {}", got.join(",")));
        }
    }
    assert!(rows > 12_000, "golden file looks truncated: {rows} rows");
    assert!(failures.is_empty(), "{} of {rows} rows differ, first ones:\n{}", failures.len(), failures[..failures.len().min(8)].join("\n"));
}

#[test]
fn hijri_matches_the_jvm_calendar() {
    let data = include_str!("data/golden_hijri.csv");
    let mut rows = 0;
    for line in data.lines().skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        let h = from_gregorian(c[0].parse().unwrap()).unwrap();
        assert_eq!((h.year.to_string().as_str(), h.month.to_string().as_str(), h.day.to_string().as_str()), (c[1], c[2], c[3]), "{}", c[0]);
        rows += 1;
    }
    assert!(rows > 600);
}

#[test]
fn every_city_zone_exists_in_the_bundled_database() {
    assert_eq!(CITIES.len(), 164);
    for city in CITIES {
        assert!(TimeZone::get(city.zone).is_ok(), "{}: {}", city.name, city.zone);
        assert!(!city.name_ar.is_empty());
    }
}

#[test]
fn next_event_follows_the_phones_sequence() {
    let o = Options::aleppo_defaults();
    let at = |s: &str| -> Timestamp { o.zone.to_zoned(s.parse::<DateTime>().unwrap()).unwrap().timestamp() };
    // Golden values for 2026-11-08: Fajr 05:33:26, sunrise 07:01:15, Maghrib 17:31:00, Isha 18:51:57.
    let e = next_event(at("2026-11-08T05:33:25"), &o);
    assert_eq!((e.prayer, fmt(e.at).as_str()), (Prayer::Fajr, "2026-11-08T05:33:26"));
    // Exactly at the moment, the next event is the following one: the phone uses isAfter(now).
    assert_eq!(next_event(at("2026-11-08T05:33:26"), &o).prayer, Prayer::Sunrise);
    assert_eq!(next_event(at("2026-11-08T17:30:59"), &o).prayer, Prayer::Maghrib);
    let after_isha = next_event(at("2026-11-08T23:00:00"), &o);
    assert_eq!(after_isha.prayer, Prayer::Fajr);
    assert_eq!(after_isha.at.date().to_string(), "2026-11-09");
}
