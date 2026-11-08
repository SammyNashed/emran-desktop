//! Saved preferences. Defaults match the Android app: Aleppo, Muslim World League, Shafi'i Asr.

use emran_core::prayer::{self, Adjustments, AsrSchool, HighLatitudeRule, Method, Options};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

pub const VOICES: [&str; 3] = ["ahmad_alnafees", "mishary_alafasy", "saad_alghamdi"];
pub const DEFAULT_LOUDNESS: u32 = 45;
const MAX_OFFSET: i32 = 120;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Offsets {
    pub fajr: i32,
    pub sunrise: i32,
    pub dhuhr: i32,
    pub asr: i32,
    pub maghrib: i32,
    pub isha: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lang: String,
    /// A preset's English name, or "custom" for typed coordinates.
    pub city: String,
    pub lat: f64,
    pub lon: f64,
    pub zone: String,
    pub method: String,
    pub school: String,
    pub high_lat: String,
    pub offsets: Offsets,
    pub voice: String,
    pub full: bool,
    /// Empty for the adhan, or the voice whose "الصلاة خير من النوم" plays at Fajr.
    pub fajr_sound: String,
    pub loudness: u32,
    pub muted: bool,
    pub clock24: bool,
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lang: "ar".into(),
            city: "Aleppo, Syria".into(),
            lat: prayer::DEFAULT_LATITUDE,
            lon: prayer::DEFAULT_LONGITUDE,
            zone: prayer::DEFAULT_ZONE_ID.into(),
            method: "MWL".into(),
            school: "SHAFII".into(),
            high_lat: "AUTOMATIC".into(),
            offsets: Offsets::default(),
            voice: VOICES[0].into(),
            full: false,
            fajr_sound: String::new(),
            loudness: DEFAULT_LOUDNESS,
            muted: false,
            clock24: true,
            autostart: false,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<Settings>(&text).ok()).unwrap_or_default().sanitized()
    }

    /// Writes to a temporary file first, so a crash mid-write cannot lose the settings.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temporary, path)
    }

    /// Replaces anything out of range, so a hand-edited file can never break the times.
    pub fn sanitized(mut self) -> Settings {
        let defaults = Settings::default();
        if self.lang != "en" {
            self.lang = defaults.lang.clone();
        }
        if !self.lat.is_finite() || !self.lon.is_finite() || self.lat.abs() > 90.0 || self.lon.abs() > 180.0 {
            self.city = defaults.city.clone();
            self.lat = defaults.lat;
            self.lon = defaults.lon;
            self.zone = defaults.zone.clone();
        }
        if jiff::tz::TimeZone::get(&self.zone).is_err() {
            self.zone = system_zone();
        }
        if Method::from_key(&self.method).is_none_or(|m| m == Method::Custom) {
            self.method = defaults.method.clone();
        }
        if AsrSchool::from_key(&self.school).is_none() {
            self.school = defaults.school.clone();
        }
        if HighLatitudeRule::from_key(&self.high_lat).is_none() {
            self.high_lat = defaults.high_lat.clone();
        }
        for value in [
            &mut self.offsets.fajr,
            &mut self.offsets.sunrise,
            &mut self.offsets.dhuhr,
            &mut self.offsets.asr,
            &mut self.offsets.maghrib,
            &mut self.offsets.isha,
        ] {
            *value = (*value).clamp(-MAX_OFFSET, MAX_OFFSET);
        }
        if !VOICES.contains(&self.voice.as_str()) {
            self.voice = defaults.voice.clone();
        }
        if self.fajr_sound != "mishary_alafasy" {
            self.fajr_sound.clear();
        }
        self.loudness = self.loudness.clamp(20, 100);
        self
    }

    pub fn options(&self) -> Options {
        Options {
            latitude: self.lat,
            longitude: self.lon,
            zone: prayer::safe_zone(Some(&self.zone)),
            method: Method::from_key(&self.method).unwrap_or(Method::Mwl),
            school: AsrSchool::from_key(&self.school).unwrap_or(AsrSchool::Shafii),
            high_latitude_rule: HighLatitudeRule::from_key(&self.high_lat).unwrap_or(HighLatitudeRule::Automatic),
            adjustments: Adjustments {
                fajr: self.offsets.fajr,
                sunrise: self.offsets.sunrise,
                dhuhr: self.offsets.dhuhr,
                asr: self.offsets.asr,
                maghrib: self.offsets.maghrib,
                isha: self.offsets.isha,
            },
            custom: None,
            use_ramadan_isha_interval: false,
        }
    }

    pub fn is_arabic(&self) -> bool {
        self.lang != "en"
    }
}

/// The computer's own IANA zone, used when coordinates are typed by hand.
pub fn system_zone() -> String {
    jiff::tz::TimeZone::system().iana_name().unwrap_or("UTC").to_string()
}
