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
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
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
#[derive(Clone, Copy)]
struct Sounds {
    wall1: &'static [u8],
    wall2: &'static [u8],
    p_paddle: &'static [u8],
    e_paddle: &'static [u8],
    miss: &'static [u8],
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
    let backend = match OutputStream::try_default() {
        Ok((stream, handle)) => Backend {
            _stream: stream,
            handle,
            sounds: Sounds {
                wall1: include_bytes!("../../assets/sounds/wallBounce1.wav"),
                wall2: include_bytes!("../../assets/sounds/wallBounce2.wav"),
                p_paddle: include_bytes!("../../assets/sounds/pPaddleBounce.wav"),
                e_paddle: include_bytes!("../../assets/sounds/ePaddleBounce.wav"),
                miss: include_bytes!("../../assets/sounds/missSound.wav"),
            },
        },
        Err(err) => {
            eprintln!("curveball: audio output unavailable, running silent: {err}");
            let _ = ready_tx.send(false);
            return;
        },
    };

    let _ = ready_tx.send(true);
    while let Ok(id) = rx.recv() {
        play(&backend, id);
    }
}

#[cfg(feature = "audio")]
fn play(backend: &Backend, id: SoundId) {
    let bytes = match id {
        SoundId::WallBounce1 => backend.sounds.wall1,
        SoundId::WallBounce2 => backend.sounds.wall2,
        SoundId::PPaddleBounce => backend.sounds.p_paddle,
        SoundId::EPaddleBounce => backend.sounds.e_paddle,
        SoundId::Miss => backend.sounds.miss,
    };
    let cursor = std::io::Cursor::new(bytes);
    let Ok(source) = Decoder::new(cursor) else {
        eprintln!("curveball: failed to decode sound effect");
        return;
    };
    if let Err(err) = backend
        .handle
        .play_raw(source.amplify(MASTER_VOLUME).convert_samples())
    {
        eprintln!("curveball: failed to play sound effect: {err}");
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
    if running_under_wsl() {
        return false;
    }
    if std::env::var_os("ALSA_CONFIG_PATH").is_some() {
        return true;
    }
    if std::fs::read_to_string("/proc/asound/cards").is_ok_and(|cards| {
        let cards = cards.trim();
        !cards.is_empty() && !cards.contains("no soundcards")
    }) {
        return true;
    }

    pulse_socket_connects() || (pipewire_socket_connects() && pipewire_alsa_config_exists())
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

#[cfg(all(feature = "audio", target_os = "linux"))]
fn running_under_wsl() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .is_ok_and(|text| text.to_ascii_lowercase().contains("microsoft"))
}
