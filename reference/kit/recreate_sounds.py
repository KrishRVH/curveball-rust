#!/usr/bin/env python3
"""Generate the modern sound set from the extracted SWF reference clips.

The faithful assets in assets/sounds/ are the ground truth: 11.025 kHz mono
PCM decoded from the MP3 streams embedded in the SWF. This script produces a
"modern" set by keeping each clip's duration, envelope, level relationship, and
one-shot shape, while rendering a cleaner 48 kHz PCM version with a restrained
transient/harmonic reconstruction pass.

Requirements: Python 3.11+ and ffmpeg built with libsoxr.
"""

from __future__ import annotations

import argparse
import math
import random
import shutil
import struct
import subprocess
import tempfile
import wave
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_DIR = ROOT / "assets" / "sounds"
OUTPUT_DIR = ROOT / "assets" / "sounds" / "modern"
TARGET_RATE = 48_000
SOURCE_RATE = 11_025
SOURCE_NYQUIST_HZ = SOURCE_RATE / 2.0
WET_CUTOFF_HZ = SOURCE_NYQUIST_HZ + 200.0
PCM_CEILING = 1.0 - 1.0 / 32768.0


@dataclass(frozen=True)
class Profile:
    """Per-clip restoration profile.

    The values are intentionally small. The dry signal remains dominant and is
    not compressed; the wet path only reconstructs missing upper-band transient
    detail above the 11.025 kHz source's Nyquist limit.
    """

    drive: float
    air: float
    transient: float
    seed: int


PROFILES: dict[str, Profile] = {
    "wallBounce1.wav": Profile(drive=2.15, air=0.030, transient=0.018, seed=101),
    "wallBounce2.wav": Profile(drive=2.10, air=0.028, transient=0.017, seed=102),
    "pPaddleBounce.wav": Profile(drive=2.35, air=0.034, transient=0.020, seed=103),
    "ePaddleBounce.wav": Profile(drive=2.25, air=0.032, transient=0.019, seed=104),
    "missSound.wav": Profile(drive=1.85, air=0.018, transient=0.010, seed=105),
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=SOURCE_DIR)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument("--ffmpeg", default="ffmpeg")
    args = parser.parse_args()

    if shutil.which(args.ffmpeg) is None:
        raise SystemExit(f"missing ffmpeg executable: {args.ffmpeg}")

    args.output_dir.mkdir(parents=True, exist_ok=True)
    for name, profile in PROFILES.items():
        source = args.source_dir / name
        output = args.output_dir / name
        render_clip(args.ffmpeg, source, output, profile)


def render_clip(ffmpeg: str, source: Path, output: Path, profile: Profile) -> None:
    with tempfile.TemporaryDirectory(prefix="curveball-audio-") as tmp_dir:
        upsampled = Path(tmp_dir) / source.name
        subprocess.run(
            [
                ffmpeg,
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-i",
                str(source),
                "-af",
                (
                    f"aresample={TARGET_RATE}:"
                    "resampler=soxr:precision=33:cheby=1:dither_method=none"
                ),
                "-ac",
                "1",
                "-ar",
                str(TARGET_RATE),
                "-sample_fmt",
                "s16",
                str(upsampled),
            ],
            check=True,
        )
        samples, rate = read_pcm16(upsampled)

    if rate != TARGET_RATE:
        raise ValueError(f"{upsampled} rendered at {rate}, expected {TARGET_RATE}")

    restored = restore(samples, profile)
    write_pcm16(output, restored, TARGET_RATE, profile.seed)
    print(summary_line(source.name, samples, restored))


def restore(samples: list[float], profile: Profile) -> list[float]:
    if not samples:
        return []

    source_rms = rms(samples)
    source_peak = peak(samples)
    dry = remove_dc(samples)

    high = high_pass(dry, cutoff_hz=2_700.0)
    excited = [math.tanh(profile.drive * x) / math.tanh(profile.drive) - x for x in high]
    excited = high_pass(excited, cutoff_hz=WET_CUTOFF_HZ)
    derivative = high_pass(differentiate(dry), cutoff_hz=WET_CUTOFF_HZ)

    shaped: list[float] = []
    for x, air, transient in zip(dry, excited, derivative, strict=True):
        shaped.append(x + profile.air * air + profile.transient * transient)

    shaped = match_rms(shaped, source_rms)
    shaped = cap_peak(shaped, min(PCM_CEILING, max(source_peak, 10 ** (-8.0 / 20.0))))
    return shaped


def remove_dc(samples: list[float]) -> list[float]:
    mean = sum(samples) / len(samples)
    return [x - mean for x in samples]


def differentiate(samples: list[float]) -> list[float]:
    out: list[float] = []
    prev = 0.0
    for x in samples:
        out.append(x - prev)
        prev = x
    return out


def high_pass(samples: list[float], cutoff_hz: float) -> list[float]:
    rc = 1.0 / (2.0 * math.pi * cutoff_hz)
    dt = 1.0 / TARGET_RATE
    alpha = rc / (rc + dt)
    out: list[float] = []
    prev_y = 0.0
    prev_x = samples[0] if samples else 0.0
    for x in samples:
        y = alpha * (prev_y + x - prev_x)
        out.append(y)
        prev_y = y
        prev_x = x
    return out


def match_rms(samples: list[float], target: float) -> list[float]:
    current = rms(samples)
    if current <= 0.0 or target <= 0.0:
        return samples
    gain = target / current
    return [x * gain for x in samples]


def cap_peak(samples: list[float], target_peak: float) -> list[float]:
    current = peak(samples)
    if current <= target_peak:
        return samples
    gain = target_peak / current
    return [x * gain for x in samples]


def rms(samples: list[float]) -> float:
    if not samples:
        return 0.0
    return math.sqrt(sum(x * x for x in samples) / len(samples))


def peak(samples: list[float]) -> float:
    return max((abs(x) for x in samples), default=0.0)


def db(value: float) -> float:
    if value <= 0.0:
        return -120.0
    return 20.0 * math.log10(value)


def summary_line(name: str, dry: list[float], restored: list[float]) -> str:
    return (
        f"{name}: {len(restored) / TARGET_RATE:.6f}s "
        f"rms {db(rms(dry)):.2f}->{db(rms(restored)):.2f} dB "
        f"peak {db(peak(dry)):.2f}->{db(peak(restored)):.2f} dB"
    )


def read_pcm16(path: Path) -> tuple[list[float], int]:
    with wave.open(str(path), "rb") as wav:
        channels = wav.getnchannels()
        sample_width = wav.getsampwidth()
        rate = wav.getframerate()
        frames = wav.readframes(wav.getnframes())
    if channels != 1 or sample_width != 2:
        raise ValueError(f"{path} must be mono 16-bit PCM")
    ints = struct.unpack(f"<{len(frames) // 2}h", frames)
    return [x / 32768.0 for x in ints], rate


def write_pcm16(path: Path, samples: list[float], rate: int, seed: int) -> None:
    rng = random.Random(seed)
    frames = bytearray()
    for x in samples:
        dither = (rng.random() - rng.random()) / 65536.0
        y = max(-1.0, min(1.0 - 1.0 / 32768.0, x + dither))
        frames.extend(struct.pack("<h", round(y * 32768.0)))
    with wave.open(str(path), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes(frames)


if __name__ == "__main__":
    main()
