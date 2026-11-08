//! Direct port of the Android app's `data/PrayerTimeCalculator.kt`.
//!
//! Keep this file in step with the Kotlin. `tests/golden.rs` compares it with output from the real
//! Kotlin file, to the second. Solar events are worked out as instants in UT and only turned into
//! wall-clock time at the end, through a real IANA zone, so daylight saving is always right.

use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

pub const DEFAULT_LATITUDE: f64 = 36.2021;
pub const DEFAULT_LONGITUDE: f64 = 37.1343;
pub const DEFAULT_ZONE_ID: &str = "Asia/Damascus";

const SUNRISE_ANGLE: f64 = 0.833;
const ALEPPO_LAT: f64 = 36.2021;
const ALEPPO_LON: f64 = 37.1343;
const ALEPPO_RADIUS_KM: f64 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Adjustments {
    pub fajr: i32,
    pub sunrise: i32,
    pub dhuhr: i32,
    pub asr: i32,
    pub maghrib: i32,
    pub isha: i32,
}

const fn temkin(sunrise: i32, dhuhr: i32, asr: i32, maghrib: i32) -> Adjustments {
    Adjustments { fajr: 0, sunrise, dhuhr, asr, maghrib, isha: 0 }
}

/// An authority's printed minute: the exact time plus its precaution in seconds, rounded up to the
/// minute for prayers and down for sunrise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfficialMinute {
    pub fajr: i64,
    pub sunrise: i64,
    pub dhuhr: i64,
    pub asr: i64,
    pub maghrib: i64,
    pub isha: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MethodParams {
    pub fajr_angle: f64,
    pub isha_angle: f64,
    pub isha_interval: i32,
    pub maghrib_angle: f64,
    pub ramadan_isha_interval: i32,
    pub temkin: Adjustments,
    pub official: Option<OfficialMinute>,
}

const fn angles(fajr_angle: f64, isha_angle: f64) -> MethodParams {
    MethodParams {
        fajr_angle,
        isha_angle,
        isha_interval: 0,
        maghrib_angle: 0.0,
        ramadan_isha_interval: 0,
        temkin: Adjustments { fajr: 0, sunrise: 0, dhuhr: 0, asr: 0, maghrib: 0, isha: 0 },
        official: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Mwl,
    Isna,
    Egyptian,
    Karachi,
    UmmAlQura,
    Dubai,
    Jordan,
    Morocco,
    Tunisia,
    Jakim,
    Kemenag,
    Qatar,
    Kuwait,
    Singapore,
    Turkey,
    Tehran,
    Jafari,
    Custom,
}

impl Method {
    /// The keys are the Android app's preference values.
    pub const ALL: [(&'static str, Method); 18] = [
        ("MWL", Method::Mwl),
        ("ISNA", Method::Isna),
        ("EGYPTIAN", Method::Egyptian),
        ("KARACHI", Method::Karachi),
        ("UMM_AL_QURA", Method::UmmAlQura),
        ("DUBAI", Method::Dubai),
        ("JORDAN", Method::Jordan),
        ("MOROCCO", Method::Morocco),
        ("TUNISIA", Method::Tunisia),
        ("JAKIM", Method::Jakim),
        ("KEMENAG", Method::Kemenag),
        ("QATAR", Method::Qatar),
        ("KUWAIT", Method::Kuwait),
        ("SINGAPORE", Method::Singapore),
        ("TURKEY", Method::Turkey),
        ("TEHRAN", Method::Tehran),
        ("JAFARI", Method::Jafari),
        ("CUSTOM", Method::Custom),
    ];

    pub fn from_key(key: &str) -> Option<Method> {
        Self::ALL.iter().find(|(k, _)| *k == key).map(|(_, m)| *m)
    }

    pub fn params(self) -> MethodParams {
        match self {
            Method::Mwl | Method::Custom => angles(18.0, 17.0),
            Method::Isna => angles(15.0, 15.0),
            Method::Egyptian => angles(19.5, 17.5),
            Method::Karachi => angles(18.0, 18.0),
            Method::UmmAlQura => MethodParams { isha_interval: 90, ramadan_isha_interval: 120, ..angles(18.5, 0.0) },
            Method::Dubai => MethodParams { temkin: temkin(0, 3, 0, 3), ..angles(18.2, 18.2) },
            Method::Jordan => MethodParams { temkin: temkin(0, 0, 0, 5), ..angles(18.0, 18.0) },
            Method::Morocco => MethodParams { temkin: temkin(0, 5, 0, 5), ..angles(19.0, 17.0) },
            Method::Tunisia => angles(18.0, 18.0),
            Method::Jakim => MethodParams {
                official: Some(OfficialMinute { fajr: 110, sunrise: -10, dhuhr: 136, asr: 80, maghrib: 40, isha: 56 }),
                ..angles(18.0, 18.0)
            },
            Method::Kemenag => MethodParams {
                official: Some(OfficialMinute { fajr: 129, sunrise: -185, dhuhr: 184, asr: 119, maghrib: 165, isha: 121 }),
                ..angles(20.0, 18.0)
            },
            Method::Qatar => MethodParams { isha_interval: 90, ..angles(18.0, 0.0) },
            Method::Kuwait => angles(18.0, 17.5),
            Method::Singapore => angles(20.0, 18.0),
            Method::Turkey => MethodParams { temkin: temkin(-7, 5, 4, 7), ..angles(18.0, 17.0) },
            Method::Tehran => MethodParams { maghrib_angle: 4.5, ..angles(17.7, 14.0) },
            Method::Jafari => MethodParams { maghrib_angle: 4.0, ..angles(16.0, 14.0) },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrSchool {
    Shafii,
    Hanafi,
}

impl AsrSchool {
    pub fn from_key(key: &str) -> Option<AsrSchool> {
        match key {
            "SHAFII" => Some(AsrSchool::Shafii),
            "HANAFI" => Some(AsrSchool::Hanafi),
            _ => None,
        }
    }

    fn shadow_ratio(self) -> f64 {
        match self {
            AsrSchool::Shafii => 1.0,
            AsrSchool::Hanafi => 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighLatitudeRule {
    None,
    MiddleOfNight,
    OneSeventhOfNight,
    AngleBased,
    Automatic,
}

impl HighLatitudeRule {
    pub fn from_key(key: &str) -> Option<HighLatitudeRule> {
        match key {
            "NONE" => Some(HighLatitudeRule::None),
            "MIDDLE_OF_NIGHT" => Some(HighLatitudeRule::MiddleOfNight),
            "ONE_SEVENTH_OF_NIGHT" => Some(HighLatitudeRule::OneSeventhOfNight),
            "ANGLE_BASED" => Some(HighLatitudeRule::AngleBased),
            "AUTOMATIC" => Some(HighLatitudeRule::Automatic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomParameters {
    pub fajr_angle: f64,
    pub isha_angle: f64,
    pub isha_interval: i32,
    pub maghrib_angle: f64,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub latitude: f64,
    pub longitude: f64,
    pub zone: TimeZone,
    pub method: Method,
    pub school: AsrSchool,
    pub high_latitude_rule: HighLatitudeRule,
    pub adjustments: Adjustments,
    pub custom: Option<CustomParameters>,
    pub use_ramadan_isha_interval: bool,
}

impl Options {
    /// The app's out-of-the-box settings: Aleppo, Muslim World League, Shafi'i Asr.
    pub fn aleppo_defaults() -> Options {
        Options {
            latitude: DEFAULT_LATITUDE,
            longitude: DEFAULT_LONGITUDE,
            zone: safe_zone(Some(DEFAULT_ZONE_ID)),
            method: Method::Mwl,
            school: AsrSchool::Shafii,
            high_latitude_rule: HighLatitudeRule::Automatic,
            adjustments: Adjustments::default(),
            custom: None,
            use_ramadan_isha_interval: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prayer {
    Fajr,
    Sunrise,
    Dhuhr,
    Asr,
    Maghrib,
    Isha,
}

impl Prayer {
    pub fn key(self) -> &'static str {
        match self {
            Prayer::Fajr => "Fajr",
            Prayer::Sunrise => "Sunrise",
            Prayer::Dhuhr => "Dhuhr",
            Prayer::Asr => "Asr",
            Prayer::Maghrib => "Maghrib",
            Prayer::Isha => "Isha",
        }
    }
}

/// Wall-clock times in `zone`, which may differ from the computer's own zone.
#[derive(Debug, Clone)]
pub struct PrayerTimes {
    pub fajr: DateTime,
    pub sunrise: DateTime,
    pub dhuhr: DateTime,
    pub asr: DateTime,
    pub maghrib: DateTime,
    pub isha: DateTime,
    pub date: Date,
    pub zone: TimeZone,
    pub has_true_sunrise_and_sunset: bool,
    pub twilight_is_astronomical: bool,
}

fn plus_minutes(t: DateTime, minutes: i32) -> DateTime {
    t.checked_add((minutes as i64).minutes()).expect("time in range")
}

fn plus_seconds(t: DateTime, seconds: i64) -> DateTime {
    t.checked_add(seconds.seconds()).expect("time in range")
}

fn truncate_to_minute(t: DateTime) -> DateTime {
    t.with().second(0).subsec_nanosecond(0).build().expect("valid time")
}

fn minute_up(t: DateTime) -> DateTime {
    let minute = truncate_to_minute(t);
    if minute == t { minute } else { plus_minutes(minute, 1) }
}

impl PrayerTimes {
    pub fn time_of(&self, prayer: Prayer) -> DateTime {
        match prayer {
            Prayer::Fajr => self.fajr,
            Prayer::Sunrise => self.sunrise,
            Prayer::Dhuhr => self.dhuhr,
            Prayer::Asr => self.asr,
            Prayer::Maghrib => self.maghrib,
            Prayer::Isha => self.isha,
        }
    }

    /// A time inside a daylight-saving gap moves forward, like Java's `atZone`.
    pub fn instant_of(&self, prayer: Prayer) -> Timestamp {
        self.zone.to_zoned(self.time_of(prayer)).expect("time in range").timestamp()
    }

    fn with_adjustments(mut self, a: Adjustments) -> PrayerTimes {
        self.fajr = plus_minutes(self.fajr, a.fajr);
        self.sunrise = plus_minutes(self.sunrise, a.sunrise);
        self.dhuhr = plus_minutes(self.dhuhr, a.dhuhr);
        self.asr = plus_minutes(self.asr, a.asr);
        self.maghrib = plus_minutes(self.maghrib, a.maghrib);
        self.isha = plus_minutes(self.isha, a.isha);
        self
    }

    fn with_official(mut self, o: OfficialMinute) -> PrayerTimes {
        self.fajr = minute_up(plus_seconds(self.fajr, o.fajr));
        self.sunrise = truncate_to_minute(plus_seconds(self.sunrise, o.sunrise));
        self.dhuhr = minute_up(plus_seconds(self.dhuhr, o.dhuhr));
        self.asr = minute_up(plus_seconds(self.asr, o.asr));
        self.maghrib = minute_up(plus_seconds(self.maghrib, o.maghrib));
        self.isha = minute_up(plus_seconds(self.isha, o.isha));
        self
    }
}

/// Parses an IANA id, falling back to the computer's zone rather than failing.
pub fn safe_zone(zone_id: Option<&str>) -> TimeZone {
    match zone_id.filter(|z| !z.trim().is_empty()) {
        Some(id) => TimeZone::get(id).unwrap_or_else(|_| TimeZone::system()),
        None => TimeZone::system(),
    }
}

pub fn calculate_times(date: Date, o: &Options) -> PrayerTimes {
    let params = o.method.params();
    let custom = o.custom.filter(|_| o.method == Method::Custom);
    let fajr_angle = custom.map_or(params.fajr_angle, |c| c.fajr_angle);
    let isha_angle = custom.map_or(params.isha_angle, |c| c.isha_angle);
    let maghrib_angle = custom.map_or(params.maghrib_angle, |c| c.maghrib_angle);
    let isha_interval = match custom {
        Some(c) => c.isha_interval,
        None if o.use_ramadan_isha_interval && params.ramadan_isha_interval > 0 => params.ramadan_isha_interval,
        None => params.isha_interval,
    };
    let (latitude, longitude) = (o.latitude, o.longitude);

    // Solar geometry, iterated so the declination is taken at the actual moment of solar noon.
    let jd0 = julian_day(date);
    let mut noon_ut = 12.0 - longitude / 15.0;
    for _ in 0..2 {
        noon_ut = 12.0 - longitude / 15.0 - solar_coordinates(jd0 + noon_ut / 24.0).equation_of_time;
    }
    let declination = solar_coordinates(jd0 + noon_ut / 24.0).declination;

    let sunrise_hour_angle = hour_angle(-SUNRISE_ANGLE, latitude, declination);
    let has_true_sunrise_and_sunset = is_attainable(-SUNRISE_ANGLE, latitude, declination);
    let sunrise_ut = noon_ut - sunrise_hour_angle;
    let sunset_ut = noon_ut + sunrise_hour_angle;

    let maghrib_ut = if maghrib_angle > 0.0 { noon_ut + hour_angle(-maghrib_angle, latitude, declination) } else { sunset_ut };

    // Asr must fall between Dhuhr and Maghrib; the clamp only binds inside the polar circles.
    let asr_altitude = (1.0 / (o.school.shadow_ratio() + tan_deg((latitude - declination).abs()))).atan().to_degrees();
    let asr_ut = coerce_in(noon_ut + hour_angle(asr_altitude, latitude, declination), noon_ut, maghrib_ut);

    let fajr_attainable = is_attainable(-fajr_angle, latitude, declination);
    let isha_attainable = is_attainable(-isha_angle, latitude, declination);
    let raw_fajr_ut = noon_ut - hour_angle(-fajr_angle, latitude, declination);
    let raw_isha_ut = noon_ut + hour_angle(-isha_angle, latitude, declination);

    let rule = resolve_rule(o.high_latitude_rule, latitude);
    let night = 24.0 - (sunset_ut - sunrise_ut);

    let fajr_ut = clamp_into_night(raw_fajr_ut, sunrise_ut - night_portion(rule, fajr_angle, night), true);
    let isha_ut = if isha_interval > 0 {
        maghrib_ut + isha_interval as f64 / 60.0
    } else {
        clamp_into_night(raw_isha_ut, sunset_ut + night_portion(rule, isha_angle, night), false)
    };

    let local = |ut: f64| to_local(date, ut, &o.zone);
    let times = PrayerTimes {
        fajr: local(fajr_ut),
        sunrise: local(sunrise_ut),
        dhuhr: local(noon_ut),
        asr: local(asr_ut),
        maghrib: local_maghrib(local(maghrib_ut), latitude, longitude, maghrib_angle),
        isha: local(isha_ut),
        date,
        zone: o.zone.clone(),
        has_true_sunrise_and_sunset,
        twilight_is_astronomical: fajr_attainable && isha_attainable,
    };
    let times = match params.official {
        Some(official) => times.with_official(official),
        None => times,
    };
    times.with_adjustments(params.temkin).with_adjustments(o.adjustments)
}

/// Aleppo's mosques call Maghrib one minute after the sunset minute (checked against real calls by
/// the Android app). Applies within 40 km of the city, and only to methods where Maghrib is sunset.
fn local_maghrib(maghrib: DateTime, latitude: f64, longitude: f64, maghrib_angle: f64) -> DateTime {
    if maghrib_angle > 0.0 || distance_km(latitude, longitude, ALEPPO_LAT, ALEPPO_LON) > ALEPPO_RADIUS_KM {
        return maghrib;
    }
    plus_minutes(minute_up(maghrib), 1)
}

fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin();
    let b = (d_lon / 2.0).sin();
    let h = a * a + lat1.to_radians().cos() * lat2.to_radians().cos() * (b * b);
    2.0 * 6371.0 * coerce_in(h, 0.0, 1.0).sqrt().asin()
}

fn resolve_rule(rule: HighLatitudeRule, latitude: f64) -> HighLatitudeRule {
    if rule == HighLatitudeRule::Automatic {
        if latitude.abs() > 48.0 { HighLatitudeRule::AngleBased } else { HighLatitudeRule::None }
    } else {
        rule
    }
}

/// How far from sunrise or sunset the rule lets Fajr or Isha sit. NaN means no limit.
fn night_portion(rule: HighLatitudeRule, angle: f64, night: f64) -> f64 {
    match rule {
        HighLatitudeRule::MiddleOfNight => night / 2.0,
        HighLatitudeRule::OneSeventhOfNight => night / 7.0,
        HighLatitudeRule::AngleBased => night * (angle / 60.0),
        HighLatitudeRule::None | HighLatitudeRule::Automatic => f64::NAN,
    }
}

fn clamp_into_night(time: f64, limit: f64, is_before: bool) -> f64 {
    if limit.is_nan() {
        time
    } else if is_before {
        time.max(limit)
    } else {
        time.min(limit)
    }
}

struct SolarCoordinates {
    declination: f64,
    equation_of_time: f64,
}

fn solar_coordinates(julian_day: f64) -> SolarCoordinates {
    let d = julian_day - 2451545.0;
    let g = fix_angle(357.529 + 0.98560028 * d);
    let q = fix_angle(280.459 + 0.98564736 * d);
    let l = fix_angle(q + 1.915 * sin_deg(g) + 0.020 * sin_deg(2.0 * g));
    let obliquity = 23.439 - 0.00000036 * d;
    let declination = (sin_deg(obliquity) * sin_deg(l)).asin().to_degrees();
    let right_ascension = fix_hour((cos_deg(obliquity) * sin_deg(l)).atan2(cos_deg(l)).to_degrees() / 15.0);
    SolarCoordinates { declination, equation_of_time: wrap_half_day(q / 15.0 - right_ascension) }
}

fn hour_angle(altitude: f64, latitude: f64, declination: f64) -> f64 {
    let c = cos_hour_angle(altitude, latitude, declination);
    if c.is_nan() {
        return 0.0;
    }
    coerce_in(c, -1.0, 1.0).acos().to_degrees() / 15.0
}

fn is_attainable(altitude: f64, latitude: f64, declination: f64) -> bool {
    let c = cos_hour_angle(altitude, latitude, declination);
    !c.is_nan() && (-1.0..=1.0).contains(&c)
}

fn cos_hour_angle(altitude: f64, latitude: f64, declination: f64) -> f64 {
    (sin_deg(altitude) - sin_deg(latitude) * sin_deg(declination)) / (cos_deg(latitude) * cos_deg(declination))
}

fn julian_day(date: Date) -> f64 {
    let mut year = date.year() as i32;
    let mut month = date.month() as i32;
    if month <= 2 {
        year -= 1;
        month += 12;
    }
    let a = (year as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (year + 4716) as f64).floor() + (30.6001 * (month + 1) as f64).floor() + date.day() as f64 + b - 1524.5
}

/// `ut_hours` is deliberately unbounded: near the antimeridian Fajr can fall on the previous UT day.
fn to_local(date: Date, ut_hours: f64, zone: &TimeZone) -> DateTime {
    if !ut_hours.is_finite() {
        return date.at(12, 0, 0, 0);
    }
    let base = TimeZone::UTC.to_zoned(date.at(0, 0, 0, 0)).expect("date in range").timestamp();
    let instant = base.checked_add(round_to_long(ut_hours * 3600.0).seconds()).expect("time in range");
    zone.to_datetime(instant)
}

/// Kotlin's `roundToLong`: halves go up, even for negative numbers (Rust's `round` goes away from zero).
fn round_to_long(x: f64) -> i64 {
    let floor = x.floor();
    (if x - floor >= 0.5 { floor + 1.0 } else { floor }) as i64
}

/// Kotlin's `coerceIn`. Rust's `clamp` panics on a NaN bound; this one never does.
fn coerce_in(v: f64, min: f64, max: f64) -> f64 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

fn sin_deg(d: f64) -> f64 {
    d.to_radians().sin()
}

fn cos_deg(d: f64) -> f64 {
    d.to_radians().cos()
}

fn tan_deg(d: f64) -> f64 {
    d.to_radians().tan()
}

fn fix_angle(a: f64) -> f64 {
    let a = a % 360.0;
    if a < 0.0 { a + 360.0 } else { a }
}

fn fix_hour(h: f64) -> f64 {
    let h = h % 24.0;
    if h < 0.0 { h + 24.0 } else { h }
}

fn wrap_half_day(h: f64) -> f64 {
    fix_hour(h + 12.0) - 12.0
}

/// The next event on the phone's Home countdown: the six daily times in order, then tomorrow's Fajr.
#[derive(Debug, Clone)]
pub struct NextEvent {
    pub prayer: Prayer,
    pub at: DateTime,
    pub instant: Timestamp,
}

pub const SEQUENCE: [Prayer; 6] = [Prayer::Fajr, Prayer::Sunrise, Prayer::Dhuhr, Prayer::Asr, Prayer::Maghrib, Prayer::Isha];

/// "Now" and the day boundary are in the location's zone, as on the phone, so someone travelling
/// with Emran pinned to their home city sees that city's countdown.
pub fn next_event(now: Timestamp, o: &Options) -> NextEvent {
    let local_now = o.zone.to_datetime(now);
    let today = calculate_times(local_now.date(), o);
    for prayer in SEQUENCE {
        if today.time_of(prayer) > local_now {
            return NextEvent { prayer, at: today.time_of(prayer), instant: today.instant_of(prayer) };
        }
    }
    let tomorrow = calculate_times(local_now.date().tomorrow().expect("date in range"), o);
    NextEvent { prayer: Prayer::Fajr, at: tomorrow.fajr, instant: tomorrow.instant_of(Prayer::Fajr) }
}
