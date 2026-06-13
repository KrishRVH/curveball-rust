//! Audio backend — the five exported sounds from frame_44, embedded as WAV.
//!
//! Flash `Sound.start(0, 1)` semantics: one playback per trigger, overlapping
//! instances allowed. Master volume is 80 % (`globalSound.setVolume(80)`).
//! A failed decode degrades to silent mode with a log line — audio is never
//! worth crashing over.

use curveball::app::SoundId;

#[cfg(feature = "audio")]
use curveball::consts::MASTER_VOLUME;
#[cfg(feature = "audio")]
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source};
#[cfg(feature = "audio")]
use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender, SyncSender},
};
#[cfg(feature = "audio")]
use std::time::Duration;

#[cfg(feature = "audio")]
pub struct Audio {
    tx: Option<Sender<SoundId>>,
}

#[cfg(feature = "audio")]
struct Backend {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sounds: Sounds,
}

#[cfg(feature = "audio")]
struct Sounds {
    wall1: DecodedSound,
    wall2: DecodedSound,
    p_paddle: DecodedSound,
    e_paddle: DecodedSound,
    miss: DecodedSound,
}

#[cfg(feature = "audio")]
struct DecodedSound {
    channels: u16,
    sample_rate: u32,
    samples: Arc<[f32]>,
}

#[cfg(feature = "audio")]
struct SoundSource {
    channels: u16,
    sample_rate: u32,
    samples: Arc<[f32]>,
    cursor: usize,
}

#[cfg(not(feature = "audio"))]
pub struct Audio;

#[cfg(feature = "audio")]
impl Audio {
    pub fn load() -> Self {
        if !should_try_audio() {
            eprintln!(
                "curveball: no audio device detected, running silent \
                 (set CURVEBALL_AUDIO=1 to force)"
            );
            return Self { tx: None };
        }

        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let spawn = std::thread::Builder::new()
            .name("curveball-audio".to_owned())
            .spawn(move || audio_thread(rx, ready_tx));
        if let Err(err) = spawn {
            eprintln!("curveball: failed to start audio thread, running silent: {err}");
            return Self { tx: None };
        }

        match ready_rx.recv_timeout(Duration::from_millis(750)) {
            Ok(true) => Self { tx: Some(tx) },
            Ok(false) => Self { tx: None },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                eprintln!("curveball: audio output probe timed out, running silent");
                Self { tx: None }
            },
            Err(err) => {
                eprintln!("curveball: audio thread did not initialize, running silent: {err}");
                Self { tx: None }
            },
        }
    }

    pub fn play(&self, id: SoundId) {
        let Some(tx) = &self.tx else { return };
        let _ = tx.send(id);
    }
}

#[cfg(feature = "audio")]
fn audio_thread(rx: Receiver<SoundId>, ready_tx: SyncSender<bool>) {
    let (stream, handle) = match OutputStream::try_default() {
        Ok(output) => output,
        Err(err) => {
            eprintln!("curveball: audio output unavailable, running silent: {err}");
            let _ = ready_tx.send(false);
            return;
        },
    };
    let Some(sounds) = Sounds::decode() else {
        eprintln!("curveball: failed to decode sound effects, running silent");
        let _ = ready_tx.send(false);
        return;
    };
    let backend = Backend {
        _stream: stream,
        handle,
        sounds,
    };

    let _ = ready_tx.send(true);
    while let Ok(id) = rx.recv() {
        play(&backend, id);
    }
}

#[cfg(feature = "audio")]
impl Sounds {
    fn decode() -> Option<Self> {
        Some(Self {
            wall1: decode_sound(include_bytes!("../../assets/sounds/wallBounce1.wav"))?,
            wall2: decode_sound(include_bytes!("../../assets/sounds/wallBounce2.wav"))?,
            p_paddle: decode_sound(include_bytes!("../../assets/sounds/pPaddleBounce.wav"))?,
            e_paddle: decode_sound(include_bytes!("../../assets/sounds/ePaddleBounce.wav"))?,
            miss: decode_sound(include_bytes!("../../assets/sounds/missSound.wav"))?,
        })
    }
}

#[cfg(feature = "audio")]
fn decode_sound(bytes: &'static [u8]) -> Option<DecodedSound> {
    let decoder = Decoder::new(std::io::Cursor::new(bytes)).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples = decoder.convert_samples::<f32>().collect::<Vec<_>>();
    (!samples.is_empty()).then(|| DecodedSound {
        channels,
        sample_rate,
        samples: Arc::from(samples),
    })
}

#[cfg(feature = "audio")]
fn play(backend: &Backend, id: SoundId) {
    let sound = match id {
        SoundId::WallBounce1 => &backend.sounds.wall1,
        SoundId::WallBounce2 => &backend.sounds.wall2,
        SoundId::PPaddleBounce => &backend.sounds.p_paddle,
        SoundId::EPaddleBounce => &backend.sounds.e_paddle,
        SoundId::Miss => &backend.sounds.miss,
    };
    if let Err(err) = backend
        .handle
        .play_raw(sound.source().amplify(MASTER_VOLUME))
    {
        eprintln!("curveball: failed to play sound effect: {err}");
    }
}

#[cfg(feature = "audio")]
impl DecodedSound {
    fn source(&self) -> SoundSource {
        SoundSource {
            channels: self.channels,
            sample_rate: self.sample_rate,
            samples: Arc::clone(&self.samples),
            cursor: 0,
        }
    }
}

#[cfg(feature = "audio")]
impl Iterator for SoundSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.samples.get(self.cursor).copied();
        self.cursor += usize::from(sample.is_some());
        sample
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.samples.len().saturating_sub(self.cursor);
        (remaining, Some(remaining))
    }
}

#[cfg(feature = "audio")]
impl Source for SoundSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.samples.len() as f64 / f64::from(self.sample_rate) / f64::from(self.channels),
        ))
    }
}

#[cfg(all(test, feature = "audio"))]
mod tests {
    #![expect(clippy::expect_used, reason = "tests assert embedded asset invariants")]

    use super::*;

    #[test]
    fn embedded_sounds_decode_to_replayable_buffers() {
        let sounds = Sounds::decode().expect("embedded wav sounds decode");
        for sound in [
            &sounds.wall1,
            &sounds.wall2,
            &sounds.p_paddle,
            &sounds.e_paddle,
            &sounds.miss,
        ] {
            assert!(sound.channels > 0);
            assert!(sound.sample_rate > 0);
            assert!(!sound.samples.is_empty());
            assert!(sound.source().total_duration().is_some());
        }
    }

    #[test]
    fn decoded_sound_sources_share_samples_and_replay_from_start() {
        let sounds = Sounds::decode().expect("embedded wav sounds decode");
        let sound = &sounds.p_paddle;

        let mut first_trigger = sound.source();
        let second_trigger = sound.source();
        assert!(Arc::ptr_eq(&first_trigger.samples, &second_trigger.samples));

        let first_samples = first_trigger.by_ref().take(16).collect::<Vec<_>>();
        let second_samples = second_trigger.take(16).collect::<Vec<_>>();

        assert_eq!(first_samples.len(), 16);
        assert_eq!(first_samples, second_samples);
        assert_eq!(
            first_trigger.size_hint(),
            (
                sound.samples.len().saturating_sub(16),
                Some(sound.samples.len().saturating_sub(16)),
            )
        );
    }
}

#[cfg(not(feature = "audio"))]
impl Audio {
    pub const fn load() -> Self {
        Self
    }

    #[expect(
        clippy::unused_self,
        reason = "no-audio facade keeps the same method interface as the audio backend"
    )]
    pub const fn play(&self, _: SoundId) {}
}

#[cfg(feature = "audio")]
fn should_try_audio() -> bool {
    if let Some(enabled) = audio_env_override() {
        return enabled;
    }

    #[cfg(target_os = "linux")]
    {
        linux_audio_route_exists()
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

#[cfg(feature = "audio")]
fn audio_env_override() -> Option<bool> {
    let value = std::env::var("CURVEBALL_AUDIO").ok()?;
    match value.to_ascii_lowercase().as_str() {
        "1" | "on" | "true" | "yes" => Some(true),
        "0" | "off" | "false" | "no" | "silent" => Some(false),
        _ => {
            eprintln!(
                "curveball: invalid CURVEBALL_AUDIO value '{value}', expected 1/0/on/off; auto-detecting"
            );
            None
        },
    }
}

#[cfg(all(feature = "audio", target_os = "linux"))]
fn linux_audio_route_exists() -> bool {
    if std::env::var_os("ALSA_CONFIG_PATH").is_some() {
        return true;
    }
    if pulse_socket_connects() {
        return true;
    }
    if pipewire_socket_connects() && pipewire_alsa_config_exists() {
        return true;
    }
    if std::fs::read_to_string("/proc/asound/cards").is_ok_and(|cards| {
        let cards = cards.trim();
        !cards.is_empty() && !cards.contains("no soundcards")
    }) {
        return true;
    }

    false
}

#[cfg(all(feature = "audio", target_os = "linux"))]
fn pulse_socket_connects() -> bool {
    if let Some(path) = std::env::var_os("PULSE_SERVER").and_then(pulse_server_path) {
        return unix_socket_connects(path);
    }
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = std::path::PathBuf::from(runtime).join("pulse/native");
        if unix_socket_connects(path) {
            return true;
        }
    }
    unix_socket_connects("/mnt/wslg/PulseServer")
}

#[cfg(all(feature = "audio", target_os = "linux"))]
fn pulse_server_path(server: std::ffi::OsString) -> Option<std::path::PathBuf> {
    let server = server.into_string().ok()?;
    server
        .strip_prefix("unix:")
        .or_else(|| server.strip_prefix("unix://"))
        .map(std::path::PathBuf::from)
}

#[cfg(all(feature = "audio", target_os = "linux"))]
fn pipewire_socket_connects() -> bool {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .is_some_and(|runtime| unix_socket_connects(runtime.join("pipewire-0")))
}

#[cfg(all(feature = "audio", target_os = "linux"))]
fn pipewire_alsa_config_exists() -> bool {
    [
        "/usr/share/alsa/alsa.conf.d/50-pipewire.conf",
        "/usr/share/alsa/alsa.conf.d/99-pipewire-default.conf",
        "/etc/alsa/conf.d/50-pipewire.conf",
        "/etc/alsa/conf.d/99-pipewire-default.conf",
    ]
    .iter()
    .any(|path| std::path::Path::new(path).exists())
}

#[cfg(all(feature = "audio", target_os = "linux"))]
fn unix_socket_connects(path: impl AsRef<std::path::Path>) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}
