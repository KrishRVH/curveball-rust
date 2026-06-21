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
use rodio::cpal::{
    self, BufferSize, FromSample, Sample, SampleFormat, SizedSample, SupportedBufferSize,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
#[cfg(feature = "audio")]
use rodio::source::UniformSourceIterator;
#[cfg(feature = "audio")]
use rodio::{Decoder, Source};
#[cfg(feature = "audio")]
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, Sender, SyncSender},
};
#[cfg(feature = "audio")]
use std::time::Duration;

#[cfg(feature = "audio")]
pub struct Audio {
    tx: Option<Sender<Playback>>,
}

#[cfg(feature = "audio")]
type Playback = SoundId;

#[cfg(feature = "audio")]
const LOW_LATENCY_BUFFER_FRAMES: u32 = 512;
#[cfg(feature = "audio")]
const LEADING_SILENCE_THRESHOLD_RATIO: f32 = 0.01;
#[cfg(feature = "audio")]
const LEADING_SILENCE_PREROLL_FRAMES: usize = 16;

#[cfg(feature = "audio")]
struct Backend {
    _stream: cpal::Stream,
    mixer: SharedMixer,
    sounds: Sounds,
}

#[cfg(feature = "audio")]
type SharedMixer = Arc<Mutex<MixerState>>;

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

#[cfg(feature = "audio")]
struct MixerState {
    channels: u16,
    sample_rate: u32,
    active: Vec<UniformSourceIterator<SoundSource, f32>>,
    pending: Vec<UniformSourceIterator<SoundSource, f32>>,
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
fn audio_thread(rx: Receiver<Playback>, ready_tx: SyncSender<bool>) {
    let mixer = Arc::new(Mutex::new(MixerState::default()));
    let stream = match open_output_stream(Arc::clone(&mixer)) {
        Ok(stream) => stream,
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
        mixer,
        sounds,
    };

    let _ = ready_tx.send(true);
    while let Ok(id) = rx.recv() {
        play(&backend, id);
    }
}

#[cfg(feature = "audio")]
fn open_output_stream(mixer: SharedMixer) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let default_device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_owned())?;

    open_device_stream(&default_device, Arc::clone(&mixer)).or_else(|default_err| {
        let devices = host
            .output_devices()
            .map_err(|err| format!("{default_err}; failed to enumerate fallback devices: {err}"))?;
        for device in devices {
            if let Ok(stream) = open_device_stream(&device, Arc::clone(&mixer)) {
                return Ok(stream);
            }
        }
        Err(default_err)
    })
}

#[cfg(feature = "audio")]
fn open_device_stream(device: &cpal::Device, mixer: SharedMixer) -> Result<cpal::Stream, String> {
    let supported = device
        .default_output_config()
        .map_err(|err| format!("failed to read default output config: {err}"))?;
    configure_mixer(&mixer, supported.channels(), supported.sample_rate().0)?;

    let low_latency_config =
        stream_config(&supported, low_latency_buffer_size(supported.buffer_size()));
    match build_and_play_stream(
        device,
        supported.sample_format(),
        &low_latency_config,
        Arc::clone(&mixer),
    ) {
        Ok(stream) => Ok(stream),
        Err(low_latency_err) => {
            let default_config = stream_config(&supported, BufferSize::Default);
            build_and_play_stream(device, supported.sample_format(), &default_config, mixer)
                .inspect(|_| {
                    eprintln!(
                        "curveball: low-latency audio buffer unavailable \
                         ({low_latency_err}); using default output buffer"
                    );
                })
        },
    }
}

#[cfg(feature = "audio")]
fn configure_mixer(mixer: &SharedMixer, channels: u16, sample_rate: u32) -> Result<(), String> {
    mixer
        .lock()
        .map_err(|_| "audio mixer lock was poisoned".to_owned())?
        .configure(channels, sample_rate);
    Ok(())
}

#[cfg(feature = "audio")]
fn stream_config(
    supported: &cpal::SupportedStreamConfig,
    buffer_size: BufferSize,
) -> cpal::StreamConfig {
    cpal::StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size,
    }
}

#[cfg(feature = "audio")]
fn low_latency_buffer_size(supported: &SupportedBufferSize) -> BufferSize {
    match *supported {
        SupportedBufferSize::Range { min, max } if min <= max => {
            BufferSize::Fixed(LOW_LATENCY_BUFFER_FRAMES.clamp(min, max))
        },
        SupportedBufferSize::Range { .. } | SupportedBufferSize::Unknown => {
            BufferSize::Fixed(LOW_LATENCY_BUFFER_FRAMES)
        },
    }
}

#[cfg(feature = "audio")]
fn build_and_play_stream(
    device: &cpal::Device,
    sample_format: SampleFormat,
    config: &cpal::StreamConfig,
    mixer: SharedMixer,
) -> Result<cpal::Stream, String> {
    let stream = build_output_stream(device, sample_format, config, mixer)
        .map_err(|err| format!("failed to build output stream: {err}"))?;
    stream
        .play()
        .map_err(|err| format!("failed to start output stream: {err}"))?;
    Ok(stream)
}

#[cfg(feature = "audio")]
fn build_output_stream(
    device: &cpal::Device,
    sample_format: SampleFormat,
    config: &cpal::StreamConfig,
    mixer: SharedMixer,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    match sample_format {
        SampleFormat::F32 => build_typed_output_stream::<f32>(device, config, mixer),
        SampleFormat::F64 => build_typed_output_stream::<f64>(device, config, mixer),
        SampleFormat::I8 => build_typed_output_stream::<i8>(device, config, mixer),
        SampleFormat::I16 => build_typed_output_stream::<i16>(device, config, mixer),
        SampleFormat::I32 => build_typed_output_stream::<i32>(device, config, mixer),
        SampleFormat::I64 => build_typed_output_stream::<i64>(device, config, mixer),
        SampleFormat::U8 => build_typed_output_stream::<u8>(device, config, mixer),
        SampleFormat::U16 => build_typed_output_stream::<u16>(device, config, mixer),
        SampleFormat::U32 => build_typed_output_stream::<u32>(device, config, mixer),
        SampleFormat::U64 => build_typed_output_stream::<u64>(device, config, mixer),
        _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
    }
}

#[cfg(feature = "audio")]
fn build_typed_output_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mixer: SharedMixer,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    device.build_output_stream::<T, _, _>(
        config,
        move |output, _| write_output(output, &mixer),
        |err| eprintln!("curveball: audio stream error: {err}"),
        None,
    )
}

#[cfg(feature = "audio")]
fn write_output<T>(output: &mut [T], mixer: &SharedMixer)
where
    T: Sample + FromSample<f32>,
{
    let Ok(mut mixer) = mixer.lock() else {
        write_silence(output);
        return;
    };
    mixer.write(output);
}

#[cfg(feature = "audio")]
fn write_silence<T>(output: &mut [T])
where
    T: Sample + FromSample<f32>,
{
    for sample in output {
        *sample = T::from_sample(0.0);
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
    let mut samples = decoder.convert_samples::<f32>().collect::<Vec<_>>();
    trim_leading_silence(&mut samples, channels);
    (!samples.is_empty()).then(|| DecodedSound {
        channels,
        sample_rate,
        samples: Arc::from(samples),
    })
}

#[cfg(feature = "audio")]
fn trim_leading_silence(samples: &mut Vec<f32>, channels: u16) {
    let Some(trim_samples) = leading_silence_trim_samples(samples, channels) else {
        return;
    };
    samples.drain(..trim_samples);
}

#[cfg(feature = "audio")]
fn leading_silence_trim_samples(samples: &[f32], channels: u16) -> Option<usize> {
    let channels = usize::from(channels.max(1));
    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max);
    if peak <= f32::EPSILON {
        return None;
    }

    let threshold = peak * LEADING_SILENCE_THRESHOLD_RATIO;
    let first_audible_frame = samples
        .chunks(channels)
        .position(|frame| frame.iter().any(|sample| sample.abs() >= threshold))?;
    let trim_frames = first_audible_frame.saturating_sub(LEADING_SILENCE_PREROLL_FRAMES);
    (trim_frames > 0).then_some(trim_frames * channels)
}

#[cfg(feature = "audio")]
fn play(backend: &Backend, id: SoundId) {
    let sounds = &backend.sounds;
    let sound = match id {
        SoundId::WallBounce1 => &sounds.wall1,
        SoundId::WallBounce2 => &sounds.wall2,
        SoundId::PPaddleBounce => &sounds.p_paddle,
        SoundId::EPaddleBounce => &sounds.e_paddle,
        SoundId::Miss => &sounds.miss,
    };
    if let Ok(mut mixer) = backend.mixer.lock() {
        mixer.play(sound);
    }
}

#[cfg(feature = "audio")]
impl Default for MixerState {
    fn default() -> Self {
        Self {
            channels: 1,
            sample_rate: 44_100,
            active: Vec::with_capacity(16),
            pending: Vec::with_capacity(16),
        }
    }
}

#[cfg(feature = "audio")]
impl MixerState {
    fn configure(&mut self, channels: u16, sample_rate: u32) {
        self.channels = channels.max(1);
        self.sample_rate = sample_rate.max(1);
        self.active.clear();
        self.pending.clear();
    }

    fn play(&mut self, sound: &DecodedSound) {
        self.pending.push(UniformSourceIterator::new(
            sound.source(),
            self.channels,
            self.sample_rate,
        ));
    }

    fn write<T>(&mut self, output: &mut [T])
    where
        T: Sample + FromSample<f32>,
    {
        let channels = usize::from(self.channels);
        for frame in output.chunks_mut(channels) {
            self.start_pending();
            for sample in frame {
                *sample = T::from_sample(self.next_sample());
            }
        }
    }

    fn start_pending(&mut self) {
        self.active.append(&mut self.pending);
    }

    fn next_sample(&mut self) -> f32 {
        let mut mixed = 0.0;
        self.active.retain_mut(|source| {
            if let Some(sample) = source.next() {
                mixed = sample.mul_add(MASTER_VOLUME, mixed);
                true
            } else {
                false
            }
        });
        mixed.clamp(-1.0, 1.0)
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

    fn decoded_test_sound(samples: &[f32]) -> DecodedSound {
        DecodedSound {
            channels: 1,
            sample_rate: 44_100,
            samples: Arc::from(samples.to_vec()),
        }
    }

    fn raw_leading_delay_ms(bytes: &'static [u8]) -> f64 {
        let decoder = Decoder::new(std::io::Cursor::new(bytes)).expect("embedded wav decodes");
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        let samples = decoder.convert_samples::<f32>().collect::<Vec<_>>();
        let trim_samples =
            leading_silence_trim_samples(&samples, channels).expect("embedded wav has attack");
        trim_samples as f64 / f64::from(channels) / f64::from(sample_rate) * 1000.0
    }

    fn decoded_leading_delay_ms(sound: &DecodedSound) -> f64 {
        let trim_samples =
            leading_silence_trim_samples(&sound.samples, sound.channels).unwrap_or(0);
        trim_samples as f64 / f64::from(sound.channels) / f64::from(sound.sample_rate) * 1000.0
    }

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
            assert_eq!(sound.channels, 1);
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

    #[test]
    fn embedded_sounds_have_large_raw_leading_silence() {
        for bytes in [
            include_bytes!("../../assets/sounds/wallBounce1.wav").as_slice(),
            include_bytes!("../../assets/sounds/wallBounce2.wav").as_slice(),
            include_bytes!("../../assets/sounds/pPaddleBounce.wav").as_slice(),
            include_bytes!("../../assets/sounds/ePaddleBounce.wav").as_slice(),
            include_bytes!("../../assets/sounds/missSound.wav").as_slice(),
        ] {
            assert!(
                raw_leading_delay_ms(bytes) > 100.0,
                "fixture should prove the original embedded wav contains perceptible silence"
            );
        }
    }

    #[test]
    fn decoded_sounds_trim_leading_silence() {
        let sounds = Sounds::decode().expect("embedded wav sounds decode");
        for sound in [
            &sounds.wall1,
            &sounds.wall2,
            &sounds.p_paddle,
            &sounds.e_paddle,
            &sounds.miss,
        ] {
            assert!(
                decoded_leading_delay_ms(sound) < 5.0,
                "decoded sound should start close to the audible attack"
            );
        }
    }

    #[test]
    fn low_latency_buffer_size_clamps_to_supported_range() {
        assert_eq!(
            low_latency_buffer_size(&SupportedBufferSize::Range {
                min: 128,
                max: 2048
            }),
            BufferSize::Fixed(LOW_LATENCY_BUFFER_FRAMES)
        );
        assert_eq!(
            low_latency_buffer_size(&SupportedBufferSize::Range {
                min: 1024,
                max: 4096
            }),
            BufferSize::Fixed(1024)
        );
        assert_eq!(
            low_latency_buffer_size(&SupportedBufferSize::Range { min: 64, max: 256 }),
            BufferSize::Fixed(256)
        );
        assert_eq!(
            low_latency_buffer_size(&SupportedBufferSize::Unknown),
            BufferSize::Fixed(LOW_LATENCY_BUFFER_FRAMES)
        );
    }

    #[test]
    fn mixer_starts_overlapping_triggers_on_next_output_frame() {
        let sound = decoded_test_sound(&[0.25, -0.5]);
        let mut mixer = MixerState::default();
        mixer.configure(2, sound.sample_rate);
        mixer.play(&sound);
        mixer.play(&sound);

        let mut output = [0.0_f32; 6];
        mixer.write(&mut output);

        let expected = [0.4, 0.4, -0.8, -0.8, 0.0, 0.0];
        for (actual, expected) in output.into_iter().zip(expected) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
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
