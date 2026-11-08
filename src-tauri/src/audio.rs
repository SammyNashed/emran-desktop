//! Adhan playback in Rust rather than the webview, so it works with the window closed and needs no
//! GStreamer. The short clips are Ogg Vorbis and the full recordings are Ogg Opus, both decoded here.

use lewton::inside_ogg::OggStreamReader;
use lewton::samples::InterleavedSamples;
use ogg::PacketReader;
use ropus::{Channels, DecodeMode, Decoder as OpusDecoder};
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, Sample, SampleRate, Source};
use std::collections::VecDeque;
use std::io::Cursor;
use std::num::NonZero;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy)]
pub enum Format {
    Vorbis,
    Opus,
}

#[derive(Clone, Copy)]
pub struct Asset {
    pub bytes: &'static [u8],
    pub format: Format,
    /// How long the recording lasts, to set its fade-in.
    pub seconds: f32,
}

macro_rules! asset {
    ($name:literal, $format:ident, $seconds:literal) => {
        Asset { bytes: include_bytes!(concat!("../audio/", $name, ".ogg")), format: Format::$format, seconds: $seconds }
    };
}

fn short_clip(voice: &str) -> Asset {
    match voice {
        "mishary_alafasy" => asset!("mishary_alafasy", Vorbis, 19.49),
        "saad_alghamdi" => asset!("saad_alghamdi", Vorbis, 4.98),
        _ => asset!("ahmad_alnafees", Vorbis, 15.36),
    }
}

fn full_adhan(voice: &str, is_fajr: bool) -> Asset {
    match voice {
        "mishary_alafasy" if is_fajr => asset!("mishary_alafasy_fajr_full", Opus, 270.51),
        "mishary_alafasy" => asset!("mishary_alafasy_full", Opus, 238.81),
        "saad_alghamdi" => asset!("saad_alghamdi_full", Opus, 117.51),
        _ => asset!("ahmad_alnafees_full", Opus, 213.61),
    }
}

/// The same choice as the phone's PrayerAlarmReceiver and AdhanPolicy.
pub fn choose(prayer_key: &str, voice: &str, full: bool, fajr_sound: &str) -> Asset {
    let is_fajr = prayer_key == "Fajr";
    if is_fajr && fajr_sound == "mishary_alafasy" {
        return asset!("mishary_alafasy_tathweeb", Opus, 35.01);
    }
    if full {
        full_adhan(voice, is_fajr)
    } else {
        short_clip(voice)
    }
}

/// Ears hear loudness on a log scale, so the slider is squared, as on the phone.
pub fn gain_for(loudness: u32) -> f32 {
    let share = loudness.clamp(20, 100) as f32 / 100.0;
    share * share
}

/// Fades in so the adhan can never be a jump-scare: a third of the clip, at most four seconds, as on the phone.
fn fade_in_for(seconds: f32) -> Duration {
    Duration::from_secs_f32((seconds / 3.0).min(4.0))
}

struct VorbisSource {
    reader: OggStreamReader<Cursor<&'static [u8]>>,
    pending: VecDeque<f32>,
}

impl Iterator for VorbisSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        while self.pending.is_empty() {
            match self.reader.read_dec_packet_generic::<InterleavedSamples<f32>>() {
                Ok(Some(packet)) => self.pending.extend(packet.samples),
                _ => return None,
            }
        }
        self.pending.pop_front()
    }
}

impl Source for VorbisSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(self.reader.ident_hdr.audio_channels as u16).unwrap_or(NonZero::<u16>::MIN)
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.reader.ident_hdr.audio_sample_rate).unwrap_or(NonZero::<u32>::MIN)
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
    fn try_seek(&mut self, _: Duration) -> Result<(), rodio::source::SeekError> {
        Err(rodio::source::SeekError::NotSupported { underlying_source: "emran vorbis" })
    }
}

/// Decodes Ogg Opus packet by packet, so a four-minute adhan never sits in memory as raw audio.
struct OpusSource {
    packets: PacketReader<Cursor<&'static [u8]>>,
    decoder: OpusDecoder,
    channels: u16,
    /// Samples per channel still to drop from the start (the encoder's pre-skip).
    skip: usize,
    decoded: u64,
    frame: Vec<i16>,
    pending: VecDeque<f32>,
}

const OPUS_RATE: u32 = 48_000;
const OPUS_MAX_FRAME: usize = 5760;

impl OpusSource {
    fn new(bytes: &'static [u8]) -> Result<OpusSource, String> {
        let mut packets = PacketReader::new(Cursor::new(bytes));
        let head = packets.read_packet().map_err(|e| e.to_string())?.ok_or("empty stream")?;
        if head.data.len() < 19 || &head.data[..8] != b"OpusHead" {
            return Err("not an Opus stream".into());
        }
        let channels = head.data[9] as u16;
        let layout = match channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            other => return Err(format!("unsupported Opus channel count {other}")),
        };
        let skip = u16::from_le_bytes([head.data[10], head.data[11]]) as usize;
        packets.read_packet().map_err(|e| e.to_string())?.ok_or("no Opus tags")?;
        let decoder = OpusDecoder::new(OPUS_RATE, layout).map_err(|e| format!("{e:?}"))?;
        Ok(OpusSource {
            packets,
            decoder,
            channels,
            skip,
            decoded: 0,
            frame: vec![0; OPUS_MAX_FRAME * channels as usize],
            pending: VecDeque::new(),
        })
    }

    fn decode_next(&mut self) -> Option<()> {
        let packet = self.packets.read_packet().ok()??;
        let per_channel = self.decoder.decode(&packet.data, &mut self.frame, DecodeMode::Normal).ok()?;
        let mut keep = per_channel;
        // The last page's position says where the audio really ends, inside its final frame.
        if packet.last_in_stream() {
            keep = keep.min(packet.absgp_page().saturating_sub(self.decoded) as usize);
        }
        self.decoded += per_channel as u64;
        let dropped = self.skip.min(keep);
        self.skip -= dropped;
        let channels = self.channels as usize;
        self.pending.extend(self.frame[dropped * channels..keep * channels].iter().map(|&v| v as f32 / 32768.0));
        Some(())
    }
}

impl Iterator for OpusSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        while self.pending.is_empty() {
            self.decode_next()?;
        }
        self.pending.pop_front()
    }
}

impl Source for OpusSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(self.channels).unwrap_or(NonZero::<u16>::MIN)
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(OPUS_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
    fn try_seek(&mut self, _: Duration) -> Result<(), rodio::source::SeekError> {
        Err(rodio::source::SeekError::NotSupported { underlying_source: "emran opus" })
    }
}

enum Command {
    Play { id: u64, asset: Asset, gain: f32 },
    Stop,
}

struct Playing {
    id: u64,
    player: Player,
    // Dropping the sink ends the sound, so it is held for as long as the adhan plays.
    _sink: MixerDeviceSink,
}

fn start(id: u64, asset: Asset, gain: f32) -> Result<Playing, String> {
    let mut sink = DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
    sink.log_on_drop(false);
    let player = Player::connect_new(sink.mixer());
    player.set_volume(gain);
    let fade = fade_in_for(asset.seconds);
    match asset.format {
        Format::Vorbis => {
            let reader = OggStreamReader::new(Cursor::new(asset.bytes)).map_err(|e| e.to_string())?;
            player.append(VorbisSource { reader, pending: VecDeque::new() }.fade_in(fade));
        }
        Format::Opus => player.append(OpusSource::new(asset.bytes)?.fade_in(fade)),
    }
    Ok(Playing { id, player, _sink: sink })
}

#[derive(Clone)]
pub struct Audio {
    commands: Sender<Command>,
}

impl Audio {
    /// `finished` runs on the audio thread when a sound ends by itself or cannot start.
    /// The sound device is only open while something plays.
    pub fn spawn(finished: impl Fn(u64) + Send + 'static) -> Audio {
        let (commands, inbox) = mpsc::channel();
        thread::spawn(move || {
            let mut current: Option<Playing> = None;
            loop {
                match inbox.recv_timeout(Duration::from_millis(200)) {
                    Ok(Command::Play { id, asset, gain }) => {
                        current = None;
                        match start(id, asset, gain) {
                            Ok(playing) => current = Some(playing),
                            Err(error) => {
                                eprintln!("adhan could not start: {error}");
                                finished(id);
                            }
                        }
                    }
                    Ok(Command::Stop) => current = None,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                if current.as_ref().is_some_and(|p| p.player.empty()) {
                    if let Some(done) = current.take() {
                        finished(done.id);
                    }
                }
            }
        });
        Audio { commands }
    }

    pub fn play(&self, id: u64, asset: Asset, gain: f32) {
        let _ = self.commands.send(Command::Play { id, asset, gain });
    }

    pub fn stop(&self) {
        let _ = self.commands.send(Command::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [(&str, Asset, f64); 8] = [
        ("ahmad short", asset!("ahmad_alnafees", Vorbis, 15.36), 15.36),
        ("ahmad full", asset!("ahmad_alnafees_full", Opus, 213.61), 213.61),
        ("mishary short", asset!("mishary_alafasy", Vorbis, 19.49), 19.49),
        ("mishary full", asset!("mishary_alafasy_full", Opus, 238.81), 238.81),
        ("mishary fajr", asset!("mishary_alafasy_fajr_full", Opus, 270.51), 270.51),
        ("mishary tathweeb", asset!("mishary_alafasy_tathweeb", Opus, 35.01), 35.01),
        ("saad short", asset!("saad_alghamdi", Vorbis, 4.98), 4.98),
        ("saad full", asset!("saad_alghamdi_full", Opus, 117.51), 117.51),
    ];

    /// Decodes each real file to the end and compares its length with what ffprobe reports.
    #[test]
    fn every_recording_decodes_to_its_real_length() {
        for (name, asset, expected) in FILES {
            let (samples, peak, rate, channels) = match asset.format {
                Format::Vorbis => {
                    let reader = OggStreamReader::new(Cursor::new(asset.bytes)).unwrap();
                    summarize(VorbisSource { reader, pending: VecDeque::new() })
                }
                Format::Opus => summarize(OpusSource::new(asset.bytes).unwrap()),
            };
            let seconds = samples as f64 / rate as f64 / channels as f64;
            assert!((seconds - expected).abs() < 0.1, "{name}: decoded {seconds:.2}s, expected {expected:.2}s");
            assert!(peak > 0.05 && peak <= 1.2, "{name}: implausible peak {peak}");
        }
    }

    fn summarize(source: impl Source) -> (u64, f32, u32, u16) {
        let (rate, channels) = (source.sample_rate().get(), source.channels().get());
        let (mut count, mut peak) = (0u64, 0f32);
        for sample in source {
            count += 1;
            peak = peak.max(sample.abs());
        }
        (count, peak, rate, channels)
    }

    /// Set EMRAN_PCM_DUMP to write the decoded audio out, to compare against ffmpeg (tools/check-audio.py).
    #[test]
    fn dump_decoded_audio_when_asked() {
        let Ok(dir) = std::env::var("EMRAN_PCM_DUMP") else { return };
        for (name, asset, _) in FILES {
            let samples: Vec<u8> = match asset.format {
                Format::Vorbis => {
                    let reader = OggStreamReader::new(Cursor::new(asset.bytes)).unwrap();
                    VorbisSource { reader, pending: VecDeque::new() }.flat_map(|s| s.to_le_bytes()).collect()
                }
                Format::Opus => OpusSource::new(asset.bytes).unwrap().flat_map(|s| s.to_le_bytes()).collect(),
            };
            std::fs::write(format!("{dir}/{}.f32", name.replace(' ', "_")), samples).unwrap();
        }
    }

    #[test]
    fn choice_matches_the_phone() {
        let same = |a: Asset, b: Asset| a.bytes == b.bytes;
        assert!(same(choose("Dhuhr", "ahmad_alnafees", false, ""), asset!("ahmad_alnafees", Vorbis, 15.36)));
        assert!(same(choose("Fajr", "ahmad_alnafees", true, ""), asset!("ahmad_alnafees_full", Opus, 213.61)));
        assert!(same(choose("Fajr", "mishary_alafasy", true, ""), asset!("mishary_alafasy_fajr_full", Opus, 270.51)));
        assert!(same(choose("Isha", "mishary_alafasy", true, ""), asset!("mishary_alafasy_full", Opus, 238.81)));
        assert!(same(choose("Fajr", "saad_alghamdi", true, "mishary_alafasy"), asset!("mishary_alafasy_tathweeb", Opus, 35.01)));
        assert!(same(choose("Maghrib", "saad_alghamdi", false, "mishary_alafasy"), asset!("saad_alghamdi", Vorbis, 4.98)));
    }
}
