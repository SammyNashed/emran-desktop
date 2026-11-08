//! The desktop shell: scheduling, tray, adhan and the bridge to the web UI.

mod audio;
mod settings;

use audio::Audio;
use emran_core::cities::CITIES;
use emran_core::hijri;
use emran_core::prayer::{calculate_times, next_event, NextEvent, Options, Prayer, PrayerTimes, SEQUENCE};
use jiff::civil::{Date, DateTime, Weekday};
use jiff::{Timestamp, ToSpan};
use serde::Serialize;
use settings::Settings;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;

/// How late an adhan may still play, for example just after the laptop wakes up. Past this it is
/// skipped rather than blasting out an hour after the prayer.
const LATE_LIMIT_SECONDS: i64 = 120;
const HIDDEN_FLAG: &str = "--hidden";

struct Tray {
    icon: TrayIcon<Wry>,
    info: MenuItem<Wry>,
    open: MenuItem<Wry>,
    stop: MenuItem<Wry>,
    mute: CheckMenuItem<Wry>,
    quit: MenuItem<Wry>,
}

struct Shared {
    settings: Mutex<Settings>,
    path: PathBuf,
    audio: Audio,
    next_sound: AtomicU64,
    /// The sound playing now, if any.
    playing: Mutex<Option<u64>>,
    /// Set when the settings change, so a new city's earlier prayer does not fire at once.
    rearm: AtomicBool,
    tray: Mutex<Option<Tray>>,
    card_title: Mutex<String>,
}

fn prayer_name(prayer: Prayer, arabic: bool, day: Date) -> &'static str {
    match (prayer, arabic) {
        (Prayer::Fajr, true) => "الفجر",
        (Prayer::Fajr, false) => "Fajr",
        (Prayer::Sunrise, true) => "الشروق",
        (Prayer::Sunrise, false) => "Sunrise",
        (Prayer::Dhuhr, true) if day.weekday() == Weekday::Friday => "الجمعة",
        (Prayer::Dhuhr, false) if day.weekday() == Weekday::Friday => "Jumu'ah",
        (Prayer::Dhuhr, true) => "الظهر",
        (Prayer::Dhuhr, false) => "Dhuhr",
        (Prayer::Asr, true) => "العصر",
        (Prayer::Asr, false) => "Asr",
        (Prayer::Maghrib, true) => "المغرب",
        (Prayer::Maghrib, false) => "Maghrib",
        (Prayer::Isha, true) => "العشاء",
        (Prayer::Isha, false) => "Isha",
    }
}

fn short_city(settings: &Settings) -> String {
    let arabic = settings.is_arabic();
    if settings.city == "custom" {
        return if arabic { "موقعك".to_string() } else { "your location".to_string() };
    }
    CITIES
        .iter()
        .find(|c| c.name == settings.city)
        .map(|c| if arabic { c.name_ar } else { c.name })
        .unwrap_or(settings.city.as_str())
        .split(['،', ','])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn clock(t: DateTime, clock24: bool, arabic: bool) -> String {
    if clock24 {
        return format!("{:02}:{:02}", t.hour(), t.minute());
    }
    let hour = if t.hour() % 12 == 0 { 12 } else { t.hour() % 12 };
    let suffix = match (t.hour() < 12, arabic) {
        (true, true) => "ص",
        (false, true) => "م",
        (true, false) => "AM",
        (false, false) => "PM",
    };
    format!("{hour}:{:02} {suffix}", t.minute())
}

#[derive(Serialize)]
struct TimeRow {
    key: &'static str,
    clock: String,
    epoch_ms: i64,
}

#[derive(Serialize)]
struct DayInfo {
    offset: i32,
    year: i16,
    month: i8,
    day: i8,
    /// 1 is Monday, 7 is Sunday.
    weekday: i8,
    hijri_year: Option<i32>,
    hijri_month: Option<u8>,
    hijri_day: Option<u8>,
    estimated: bool,
    times: Vec<TimeRow>,
}

#[derive(Serialize)]
struct NextInfo {
    key: &'static str,
    clock: String,
    epoch_ms: i64,
    weekday: i8,
}

#[derive(Serialize)]
struct Snapshot {
    settings: Settings,
    next: NextInfo,
    today: DayInfo,
    shown: DayInfo,
    tomorrow_fajr_ms: i64,
    playing: bool,
}

fn day_info(offset: i32, times: &PrayerTimes, settings: &Settings) -> DayInfo {
    let arabic = settings.is_arabic();
    let hijri = hijri::from_gregorian(times.date);
    DayInfo {
        offset,
        year: times.date.year(),
        month: times.date.month(),
        day: times.date.day(),
        weekday: times.date.weekday().to_monday_one_offset(),
        hijri_year: hijri.map(|h| h.year),
        hijri_month: hijri.map(|h| h.month),
        hijri_day: hijri.map(|h| h.day),
        estimated: !times.has_true_sunrise_and_sunset || !times.twilight_is_astronomical,
        times: SEQUENCE
            .iter()
            .map(|&p| TimeRow { key: p.key(), clock: clock(times.time_of(p), settings.clock24, arabic), epoch_ms: times.instant_of(p).as_millisecond() })
            .collect(),
    }
}

fn today_in_zone(options: &Options) -> Date {
    options.zone.to_datetime(Timestamp::now()).date()
}

fn add_days(date: Date, days: i32) -> Date {
    date.checked_add(days.days()).unwrap_or(date)
}

fn snapshot(shared: &Shared, offset: i32) -> Snapshot {
    let settings = shared.settings.lock().unwrap().clone();
    let options = settings.options();
    let now = Timestamp::now();
    let today_date = today_in_zone(&options);
    let today = calculate_times(today_date, &options);
    let shown = calculate_times(add_days(today_date, offset), &options);
    let tomorrow = calculate_times(add_days(today_date, 1), &options);
    let next = next_event(now, &options);
    Snapshot {
        next: NextInfo {
            key: next.prayer.key(),
            clock: clock(next.at, settings.clock24, settings.is_arabic()),
            epoch_ms: next.instant.as_millisecond(),
            weekday: next.at.date().weekday().to_monday_one_offset(),
        },
        today: day_info(0, &today, &settings),
        shown: day_info(offset, &shown, &settings),
        tomorrow_fajr_ms: tomorrow.instant_of(Prayer::Fajr).as_millisecond(),
        playing: shared.playing.lock().unwrap().is_some(),
        settings,
    }
}

fn notify_ui(app: &AppHandle) {
    let _ = app.emit("changed", ());
}

fn tray_line(settings: &Settings, next: &NextEvent, now: Timestamp) -> String {
    let arabic = settings.is_arabic();
    let seconds = next.instant.as_second() - now.as_second();
    // Rounded up, so it never reads 00:00 while the prayer is still ahead.
    let minutes = (seconds + 59) / 60;
    let (hours, minutes) = (minutes / 60, minutes % 60);
    let name = prayer_name(next.prayer, arabic, next.at.date());
    let at = clock(next.at, settings.clock24, arabic);
    if arabic {
        format!("{name} {at} · باقي \u{2066}{hours:02}:{minutes:02}\u{2069}")
    } else {
        format!("{name} {at} · in {hours:02}:{minutes:02}")
    }
}

fn refresh_tray_labels(shared: &Shared) {
    let settings = shared.settings.lock().unwrap().clone();
    let playing = shared.playing.lock().unwrap().is_some();
    if let Some(tray) = shared.tray.lock().unwrap().as_ref() {
        let arabic = settings.is_arabic();
        let _ = tray.open.set_text(if arabic { "فتح عِمران" } else { "Open Emran" });
        let _ = tray.stop.set_text(if arabic { "إيقاف الأذان" } else { "Stop Adhan" });
        let _ = tray.stop.set_enabled(playing);
        let _ = tray.mute.set_text(if arabic { "كتم الأذان" } else { "Mute adhan" });
        let _ = tray.mute.set_checked(settings.muted);
        let _ = tray.quit.set_text(if arabic { "خروج" } else { "Quit" });
    }
}

fn close_card(app: &AppHandle) {
    if let Some(card) = app.get_webview_window("adhan") {
        let _ = card.destroy();
    }
}

fn stop_sound(app: &AppHandle) {
    let shared = app.state::<Shared>();
    shared.audio.stop();
    *shared.playing.lock().unwrap() = None;
    close_card(app);
    refresh_tray_labels(&shared);
    notify_ui(app);
}

fn sound_finished(app: &AppHandle, id: u64) {
    let shared = app.state::<Shared>();
    let mut playing = shared.playing.lock().unwrap();
    if *playing != Some(id) {
        return;
    }
    *playing = None;
    drop(playing);
    close_card(app);
    refresh_tray_labels(&shared);
    notify_ui(app);
}

fn start_sound(app: &AppHandle, prayer_key: &str) {
    let shared = app.state::<Shared>();
    let settings = shared.settings.lock().unwrap().clone();
    let asset = audio::choose(prayer_key, &settings.voice, settings.full, &settings.fajr_sound);
    let id = shared.next_sound.fetch_add(1, Ordering::Relaxed);
    *shared.playing.lock().unwrap() = Some(id);
    shared.audio.play(id, asset, audio::gain_for(settings.loudness));
    refresh_tray_labels(&shared);
    notify_ui(app);
}

/// A small always-on-top card that does not take focus, because desktop notifications can't hold a Stop button.
fn show_card(app: &AppHandle, title: String) {
    *app.state::<Shared>().card_title.lock().unwrap() = title;
    close_card(app);
    const WIDTH: f64 = 460.0;
    const HEIGHT: f64 = 196.0;
    const MARGIN: f64 = 24.0;
    let built = WebviewWindowBuilder::new(app, "adhan", WebviewUrl::App("adhan.html".into()))
        .title("عِمران")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .build();
    let card = match built {
        Ok(card) => card,
        Err(error) => {
            eprintln!("stop card could not open: {error}");
            return;
        }
    };
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size();
        let origin = monitor.position();
        let x = origin.x as f64 + size.width as f64 - (WIDTH + MARGIN) * scale;
        let y = origin.y as f64 + size.height as f64 - (HEIGHT + MARGIN * 2.0) * scale;
        let _ = card.set_position(PhysicalPosition::new(x as i32, y as i32));
    }
}

fn fire_adhan(app: &AppHandle, prayer: Prayer, day: Date) {
    let settings = app.state::<Shared>().settings.lock().unwrap().clone();
    let arabic = settings.is_arabic();
    let name = prayer_name(prayer, arabic, day);
    let city = short_city(&settings);
    let (title, body) = if arabic {
        (format!("صلاة {name}"), format!("حان الآن وقت صلاة {name} في {city}"))
    } else {
        (format!("{name} Prayer"), format!("It is now time for {name} prayer in {city}"))
    };
    if let Err(error) = app.notification().builder().title(&title).body(&body).show() {
        eprintln!("notification failed: {error}");
    }
    if !settings.muted {
        start_sound(app, prayer.key());
        show_card(app, title);
    }
}

/// The next of the five prayers after `cursor`. Sunrise never gets an adhan, as on the phone.
fn next_adhan(cursor: Timestamp, options: &Options) -> NextEvent {
    let mut from = cursor;
    loop {
        let event = next_event(from, options);
        if event.prayer != Prayer::Sunrise {
            return event;
        }
        from = event.instant;
    }
}

/// Checks the wall clock twice a second instead of sleeping until the prayer, so it stays right
/// after suspend, a clock change or a new city.
fn run_scheduler(app: AppHandle) {
    let mut cursor = Timestamp::now();
    let mut last_line = String::new();
    let mut last_day: Option<Date> = None;
    let mut last_next = 0i64;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let shared = app.state::<Shared>();
        let settings = shared.settings.lock().unwrap().clone();
        let options = settings.options();
        let now = Timestamp::now();

        let clock_went_back = cursor.as_second() - now.as_second() > 5;
        if shared.rearm.swap(false, Ordering::SeqCst) || clock_went_back {
            cursor = now;
        }
        loop {
            let event = next_adhan(cursor, &options);
            if event.instant > now {
                break;
            }
            cursor = event.instant;
            if now.as_second() - event.instant.as_second() <= LATE_LIMIT_SECONDS {
                fire_adhan(&app, event.prayer, event.at.date());
            }
        }

        let next = next_event(now, &options);
        let day = options.zone.to_datetime(now).date();
        if last_day != Some(day) || last_next != next.instant.as_second() {
            last_day = Some(day);
            last_next = next.instant.as_second();
            notify_ui(&app);
        }
        let line = tray_line(&settings, &next, now);
        if line != last_line {
            if let Some(tray) = shared.tray.lock().unwrap().as_ref() {
                let _ = tray.info.set_text(&line);
                let _ = tray.icon.set_tooltip(Some(line.clone()));
            }
            last_line = line;
        }
    }
}

fn store_settings(app: &AppHandle, settings: Settings) {
    let shared = app.state::<Shared>();
    if let Err(error) = settings.save(&shared.path) {
        eprintln!("settings could not be saved: {error}");
    }
    *shared.settings.lock().unwrap() = settings;
    shared.rearm.store(true, Ordering::SeqCst);
    refresh_tray_labels(&shared);
    notify_ui(app);
}

fn set_muted(app: &AppHandle, muted: bool) {
    let mut settings = app.state::<Shared>().settings.lock().unwrap().clone();
    settings.muted = muted;
    store_settings(app, settings);
    if muted {
        stop_sound(app);
    }
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn get_snapshot(shared: State<Shared>, offset: i32) -> Snapshot {
    snapshot(&shared, offset.clamp(-365, 365))
}

#[derive(Serialize)]
struct CityRow {
    name: &'static str,
    name_ar: &'static str,
    lat: f64,
    lon: f64,
    zone: &'static str,
}

#[tauri::command]
fn get_cities() -> Vec<CityRow> {
    CITIES.iter().map(|c| CityRow { name: c.name, name_ar: c.name_ar, lat: c.lat, lon: c.lon, zone: c.zone }).collect()
}

#[tauri::command]
fn system_zone() -> String {
    settings::system_zone()
}

/// Autostart has its own command, because it must change the operating system first.
#[tauri::command]
fn update_settings(app: AppHandle, shared: State<Shared>, new: Settings) {
    let autostart = shared.settings.lock().unwrap().autostart;
    store_settings(&app, Settings { autostart, ..new }.sanitized());
}

#[tauri::command]
fn set_autostart(app: AppHandle, shared: State<Shared>, on: bool) -> bool {
    let launcher = app.autolaunch();
    let result = if on { launcher.enable() } else { launcher.disable() };
    if let Err(error) = result {
        eprintln!("autostart change failed: {error}");
    }
    let actual = launcher.is_enabled().unwrap_or(false);
    let mut settings = shared.settings.lock().unwrap().clone();
    settings.autostart = actual;
    store_settings(&app, settings);
    actual
}

#[tauri::command]
fn toggle_mute(app: AppHandle, shared: State<Shared>) {
    let muted = shared.settings.lock().unwrap().muted;
    set_muted(&app, !muted);
}

#[tauri::command]
fn preview_adhan(app: AppHandle, prayer: String) {
    let key = if prayer == "Fajr" { "Fajr" } else { "Maghrib" };
    start_sound(&app, key);
}

#[tauri::command]
fn stop_adhan(app: AppHandle) {
    stop_sound(&app);
}

#[derive(Serialize)]
struct CardText {
    title: String,
    stop: &'static str,
    arabic: bool,
}

#[tauri::command]
fn get_card(shared: State<Shared>) -> CardText {
    let arabic = shared.settings.lock().unwrap().is_arabic();
    CardText { title: shared.card_title.lock().unwrap().clone(), stop: if arabic { "إيقاف الأذان" } else { "Stop Adhan" }, arabic }
}

fn build_tray(app: &AppHandle) -> tauri::Result<Tray> {
    let info = MenuItem::with_id(app, "info", "…", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Stop", false, None::<&str>)?;
    let mute = CheckMenuItem::with_id(app, "mute", "Mute", true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let first = PredefinedMenuItem::separator(app)?;
    let second = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&info, &first, &open, &stop, &mute, &second, &quit])?;
    let icon = TrayIconBuilder::with_id("emran")
        .icon(app.default_window_icon().cloned().expect("bundled icon"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "stop" => stop_sound(app),
            "mute" => {
                let muted = app.state::<Shared>().settings.lock().unwrap().muted;
                set_muted(app, !muted);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(Tray { icon, info, open, stop, mute, quit })
}

pub fn run() {
    // WebKitGTK's DMABUF renderer leaves the window blank on many Linux setups. This page barely
    // animates, so nothing is lost by turning it off.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_FLAG])))
        .setup(|app| {
            let handle = app.handle().clone();
            let path = app.path().app_config_dir()?.join("settings.json");
            let mut loaded = Settings::load(&path);
            loaded.autostart = app.autolaunch().is_enabled().unwrap_or(false);
            let finished_handle = handle.clone();
            let audio = Audio::spawn(move |id| sound_finished(&finished_handle, id));
            app.manage(Shared {
                settings: Mutex::new(loaded),
                path,
                audio,
                next_sound: AtomicU64::new(1),
                playing: Mutex::new(None),
                rearm: AtomicBool::new(false),
                tray: Mutex::new(None),
                card_title: Mutex::new(String::new()),
            });
            let tray = build_tray(&handle)?;
            let shared = app.state::<Shared>();
            *shared.tray.lock().unwrap() = Some(tray);
            refresh_tray_labels(&shared);
            if !std::env::args().any(|arg| arg == HIDDEN_FLAG) {
                show_main(&handle);
            }
            let scheduler_handle = handle.clone();
            std::thread::spawn(move || run_scheduler(scheduler_handle));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                } else if window.label() == "adhan" {
                    stop_sound(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_cities,
            system_zone,
            update_settings,
            set_autostart,
            toggle_mute,
            preview_adhan,
            stop_adhan,
            get_card
        ])
        .run(tauri::generate_context!())
        .expect("error while running Emran");
}
