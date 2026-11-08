# Emran Desktop (عِمران)

Prayer times and adhan for Windows and Linux. The desktop companion to [Emran for Android](https://github.com/SammyNashed/Emran).

- Fully offline. The app makes no network calls and collects nothing.
- Prayer times computed on your computer with the same maths and defaults as the Android app.
- Arabic first, with an English toggle.
- Lives in the tray with the next prayer and its countdown. Closing the window keeps it running.
- The adhan plays at prayer time with a quiet notification and a one-click Stop card.
- Reciters: Ahmad Al-Nafees, Mishary Alafasy and Saad Al-Ghamdi, short or full.

## Install

Windows builds are unsigned, so SmartScreen may warn the first time: choose **More info**, then **Run anyway**.

On Linux, make the AppImage executable and run it, or install the `.deb`.

## Build from source

You need Rust, Node.js and, on Linux, the WebKitGTK 4.1, GTK 3, ayatana-appindicator and ALSA development packages.

```sh
npm ci
npx tauri build
```

## How the prayer times are checked

`core/src/prayer.rs` is a direct port of the Android app's `PrayerTimeCalculator.kt`. `tools/golden/run.sh` compiles that Kotlin file and dumps its output, and `cargo test -p emran-core` compares the port with it to the second across every calculation method, Asr school and high-latitude rule, plus two full years for Aleppo.

## License

GPL-3.0. See [LICENSE](LICENSE).
