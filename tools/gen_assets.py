#!/usr/bin/env python3
"""RAILGUN PURSUIT 3D — procedural asset generator (Module 5).

Deterministic. Run from the repository root:

    python3 tools/gen_assets.py

Generates (created under assets/, git-ignored — never committed):

  textures/asphalt_4k.jpg    4096x4096 asphalt albedo      (road surface)
  textures/sand_4k.jpg       4096x4096 sand albedo         (wasteland/swamp ground)
  textures/concrete_4k.jpg   4096x4096 concrete albedo     (ruined-city ground)
  sfx/*.wav                  44.1 kHz 16-bit mono one-shots + seamless loops
  music/industry_legend_5min.wav   5:00 industrial loop, stereo (the 5-min track)

Everything is synthesized (no network, no external masters). The generated
files are REAL assets the game loads and uses — but they are procedural
placeholders, not hand-painted 4K PBR art or a mastered industrial track.
See README.md ("Honesty") before believing any "budget" number.

Writes assets/manifest.json listing real paths + real byte sizes (consumed by
build.rs to render the loading screen's real progress bar).
"""

import json
import struct
import sys
import wave
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
TEX = ROOT / "assets" / "textures"
SFX = ROOT / "assets" / "audio"
MUS = ROOT / "assets" / "music"
SR = 44100
S = 4096  # texture edge length (4K)

WRITTEN: list[tuple[str, int]] = []


def record(path: Path) -> None:
    rel = path.relative_to(ROOT / "assets")
    WRITTEN.append((str(rel).replace("\\", "/"), path.stat().st_size))


# ---------------------------------------------------------------- textures

def fbm(size: int, octaves: int, seed: int, grid_start: int = 4, gain: float = 0.5) -> np.ndarray:
    """Fractal value noise in [0, 1]. Smooth, bilinearly interpolated octaves."""
    rng = np.random.default_rng(seed)
    total = np.zeros((size, size), dtype=np.float32)
    amp, norm, grid = 1.0, 0.0, grid_start
    for _ in range(octaves):
        noise = rng.random((grid, grid), dtype=np.float32)
        img = Image.fromarray((noise * 255).astype(np.uint8), mode="L").resize(
            (size, size), Image.BILINEAR
        )
        total += (np.asarray(img, dtype=np.float32) / 255.0) * amp
        norm += amp
        amp *= gain
        grid *= 2
    return total / norm


def save_jpg(path: Path, rgb: np.ndarray, quality: int = 88) -> None:
    arr = np.clip(rgb * 255.0, 0, 255).astype(np.uint8)
    Image.fromarray(arr, mode="RGB").save(path, "JPEG", quality=quality)
    record(path)
    print(f"  tex  {path.name:24s} {path.stat().st_size / 1e6:8.2f} MB")


def speckle(base: np.ndarray, rng: np.random.Generator, bright: float, dark: float) -> np.ndarray:
    r = rng.random(base.shape)
    base = np.where(r > 0.99930, bright, base)
    base = np.where(r < 0.00070, dark, base)
    return base


def gen_asphalt() -> None:
    rng = np.random.default_rng(9001)
    h = fbm(S, 6, seed=11, grid_start=4)
    base = 0.115 + h * 0.125
    patches = fbm(S, 4, seed=12, grid_start=32)
    base *= 0.82 + 0.36 * patches
    base = speckle(base, rng, 0.52, 0.030)
    yy, xx = np.mgrid[0:S, 0:S]
    for _ in range(9):  # oil stains
        cx, cy = rng.uniform(0, S, 2)
        sigma = rng.uniform(120.0, 380.0)
        amp = rng.uniform(0.020, 0.055)
        base -= amp * np.exp(-(((xx - cx) ** 2 + (yy - cy) ** 2) / (2.0 * sigma * sigma))).astype(np.float32)
    base = np.clip(base, 0.0, 1.0)
    rgb = np.dstack([base * 1.015, base * 1.000, base * 0.975])
    save_jpg(TEX / "asphalt_4k.jpg", rgb)


def gen_sand() -> None:
    a = 0.42  # ripple direction
    yy, xx = np.mgrid[0:S, 0:S].astype(np.float32)
    h = fbm(S, 6, seed=21, grid_start=4)
    dune = fbm(S, 3, seed=22, grid_start=2)
    along = (xx * np.cos(a) + yy * np.sin(a)) / 24.0
    ripple = 0.5 + 0.5 * np.sin(along + h * 6.0)
    base = 0.50 + 0.26 * h + 0.055 * ripple + 0.12 * dune
    rng = np.random.default_rng(9002)
    base = speckle(base, rng, 0.86, 0.28)
    base = np.clip(base, 0.0, 1.0)
    rgb = np.dstack([base * 1.13, base * 0.955, base * 0.660])
    save_jpg(TEX / "sand_4k.jpg", rgb)


def gen_concrete() -> None:
    h = fbm(S, 6, seed=31, grid_start=4)
    base = 0.355 + 0.235 * h
    stain = fbm(S, 4, seed=32, grid_start=8)
    base = np.where(stain > 0.72, base * 0.80, base)  # dark water stains
    ridge = np.abs(fbm(S, 6, seed=33, grid_start=48) - 0.5) * 2.0
    base = np.where(ridge < 0.016, base * 0.55, base)  # hairline shrinkage cracks
    seam_idx = np.arange(S) % 1024 < 4
    base[seam_idx, :] *= 0.55
    base[:, seam_idx] *= 0.55
    rng = np.random.default_rng(9003)
    base = speckle(base, rng, 0.62, 0.10)
    base = np.clip(base, 0.0, 1.0)
    rgb = np.dstack([base * 0.985, base * 1.000, base * 0.935])
    save_jpg(TEX / "concrete_4k.jpg", rgb)


# -------------------------------------------------------------------- wav

def write_wav(path: Path, samples: np.ndarray, stereo: bool = False) -> None:
    pcm = (np.clip(samples, -1.0, 1.0) * 32767.0).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(2 if stereo else 1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    record(path)
    print(f"  wav  {path.name:34s} {path.stat().st_size / 1e6:8.2f} MB")


def t_axis(seconds: float) -> np.ndarray:
    return np.arange(int(seconds * SR), dtype=np.float32) / SR


def gen_engine_loop() -> None:
    t = t_axis(2.0)  # integer cycles for seamless loop
    sig = np.zeros_like(t)
    for freq, amp in ((38.0, 1.0), (76.0, 0.62), (114.0, 0.38), (152.0, 0.24), (228.0, 0.12)):
        sig += amp * np.sin(2.0 * np.pi * freq * t)
    sig *= 0.74 + 0.26 * np.sin(2.0 * np.pi * 7.0 * t)  # 7 Hz idle lope (14 cycles)
    rng = np.random.default_rng(4242)
    sig += rng.standard_normal(t.shape).astype(np.float32) * 0.045
    sig /= np.max(np.abs(sig)) * 1.9
    write_wav(SFX / "engine_loop.wav", sig)


def gen_wind_loop() -> None:
    """Perfectly looping band-shaped noise via irfft of a Hermitian spectrum."""
    n = int(3.0 * SR)  # 3.0 s, all LFOs complete integer cycles
    rng = np.random.default_rng(777)
    half = n // 2 + 1
    freqs = np.fft.rfftfreq(n, 1.0 / SR)
    magnitude = 1.0 / np.sqrt(freqs + 18.0)
    # form a 300..750 Hz hump on top of the pink tilt
    magnitude *= 0.55 + 0.9 * np.exp(-((freqs - 480.0) ** 2) / (2.0 * 210.0**2))
    phase = rng.uniform(0.0, 2.0 * np.pi, half)
    spec = np.zeros(half, dtype=np.complex128)
    spec[1:] = magnitude[1:] * np.exp(1j * phase[1:])
    sig = np.fft.irfft(spec, n).astype(np.float32)
    sig /= np.max(np.abs(sig))
    t = np.arange(n, dtype=np.float32) / SR
    sig *= 0.52 + 0.28 * np.sin(2.0 * np.pi * (5.0 / 3.0) * t) + 0.14 * np.sin(2.0 * np.pi * (8.0 / 3.0) * t + 1.3)
    sig /= np.max(np.abs(sig)) * 1.55
    write_wav(SFX / "wind_loop.wav", sig)


def sfx_tone_burst(path: Path, seconds: float, partials: list[tuple[float, float, float]],
                   noise_amp: float = 0.0, noise_decay: float = 0.05, peak: float = 0.9,
                   seed: int = 99) -> None:
    """partials: list of (freq_hz, amp, decay_s)."""
    t = t_axis(seconds)
    sig = np.zeros_like(t)
    for freq, amp, decay in partials:
        sig += amp * np.sin(2.0 * np.pi * freq * t) * np.exp(-t / decay)
    if noise_amp > 0.0:
        rng = np.random.default_rng(seed)
        sig += rng.standard_normal(t.shape).astype(np.float32) * noise_amp * np.exp(-t / noise_decay)
    sig /= np.max(np.abs(sig)) / peak
    write_wav(path, sig)


def gen_shot_ap() -> None:
    sfx_tone_burst(SFX / "shot_ap.wav", 0.18,
                   [(55.0, 1.0, 0.055), (2100.0, 0.55, 0.020), (4700.0, 0.30, 0.012), (7300.0, 0.18, 0.008)],
                   noise_amp=0.75, noise_decay=0.030, peak=0.92, seed=5150)


def gen_shot_inc() -> None:
    sfx_tone_burst(SFX / "shot_inc.wav", 0.32,
                   [(42.0, 1.0, 0.085), (1200.0, 0.32, 0.055), (340.0, 0.35, 0.045)],
                   noise_amp=0.55, noise_decay=0.075, peak=0.88, seed=6161)


def gen_explosion_big() -> None:
    t = t_axis(1.7)
    sig = np.zeros_like(t)
    rng = np.random.default_rng(7171)
    brown = np.cumsum(rng.standard_normal(t.shape)).astype(np.float32)
    brown /= np.max(np.abs(brown))
    sig += brown * np.exp(-t / 0.55) * 0.9
    for delay, amp in ((0.0, 1.0), (0.28, 0.62), (0.62, 0.38)):
        seg = t - delay
        mask = seg >= 0
        sig += np.where(mask, np.sin(2.0 * np.pi * 29.0 * np.clip(seg, 0, None)) * np.exp(-np.clip(seg, 0, None) / 0.16) * amp, 0.0).astype(np.float32)
    sig += np.sin(2.0 * np.pi * 74.0 * t) * np.exp(-t / 0.30) * 0.5
    sig /= np.max(np.abs(sig)) / 0.97
    write_wav(SFX / "explosion_big.wav", sig)


def gen_explosion_small() -> None:
    t = t_axis(0.75)
    rng = np.random.default_rng(8181)
    noise = rng.standard_normal(t.shape).astype(np.float32)
    low = np.convolve(noise, np.ones(24, np.float32) / 24.0, mode="same")
    sig = noise * 0.45 * np.exp(-t / 0.07) + np.sin(2.0 * np.pi * 46.0 * t) * np.exp(-t / 0.13) * 0.9 + low * np.exp(-t / 0.22) * 0.35
    sig /= np.max(np.abs(sig)) / 0.92
    write_wav(SFX / "explosion_small.wav", sig)


def gen_ricochet() -> None:
    t = t_axis(0.24)
    chirp = 2900.0 + 1400.0 * (1.0 - np.exp(-t / 0.05))
    sig = np.sin(2.0 * np.pi * np.cumsum(chirp) / SR) * np.exp(-t / 0.075) * 0.8
    sig += np.sin(2.0 * np.pi * 6100.0 * t) * np.exp(-t / 0.030) * 0.35
    sig /= np.max(np.abs(sig)) / 0.85
    write_wav(SFX / "ricochet.wav", sig)


def gen_pickup_scrap() -> None:
    sfx_tone_burst(SFX / "pickup_scrap.wav", 0.10,
                   [(185.0, 1.0, 0.030), (620.0, 0.45, 0.018)],
                   noise_amp=0.35, noise_decay=0.012, peak=0.7, seed=9191)


def gen_pickup_medkit() -> None:
    t = t_axis(0.38)
    note1 = np.sin(2.0 * np.pi * 660.0 * t) * np.exp(-t / 0.16)
    note2 = np.sin(2.0 * np.pi * 990.0 * t) * np.exp(-np.maximum(0.0, t - 0.09) / 0.14)
    note2 = np.where(t >= 0.09, note2, 0.0)
    sig = (note1 * 0.8 + note2 * 0.8).astype(np.float32)
    sig /= np.max(np.abs(sig)) / 0.7
    write_wav(SFX / "pickup_medkit.wav", sig)


def gen_overheat() -> None:
    t = t_axis(0.52)
    phase = 2.0 * np.pi * 1240.0 * t
    square = np.sign(np.sin(phase))
    gate = ((t < 0.12) | ((t > 0.19) & (t < 0.31))).astype(np.float32)
    sig = (square * gate * 0.75).astype(np.float32)
    write_wav(SFX / "overheat.wav", sig)


def gen_mine_beep() -> None:
    sfx_tone_burst(SFX / "mine_beep.wav", 0.09, [(1500.0, 1.0, 0.030)], peak=0.55, seed=1111)


def gen_ram_crash() -> None:
    t = t_axis(0.85)
    rng = np.random.default_rng(1212)
    noise = rng.standard_normal(t.shape).astype(np.float32)
    sig = np.sin(2.0 * np.pi * 55.0 * t) * np.exp(-t / 0.16) * 1.0
    for freq, amp, decay in ((221.0, 0.62, 0.16), (417.0, 0.42, 0.11), (611.0, 0.30, 0.075)):
        sig += amp * np.sin(2.0 * np.pi * freq * t) * np.exp(-t / decay)
    sig += noise * 0.5 * np.exp(-t / 0.045)
    sig /= np.max(np.abs(sig)) / 0.95
    write_wav(SFX / "ram_crash.wav", sig)


def gen_workbench() -> None:
    t = t_axis(0.20)
    sig = np.sin(2.0 * np.pi * 880.0 * t) * np.exp(-t / 0.07) * 0.7
    sig += np.sin(2.0 * np.pi * 1320.0 * t) * np.exp(-t / 0.05) * 0.4
    sig /= np.max(np.abs(sig)) / 0.65
    write_wav(SFX / "workbench.wav", sig)


# ------------------------------------------------------------------ music

def gen_music() -> None:
    """INDUSTRY LEGEND — 5:00, 100 BPM, A-minor industrial. Deterministic."""
    rng = np.random.default_rng(1337)
    bpm = 100.0
    spb = 60.0 / bpm
    bar = 4.0 * spb
    bars = 125  # 300.0 s exactly
    n_total = int(bars * bar * SR)
    left = np.zeros(n_total, dtype=np.float32)
    right = np.zeros(n_total, dtype=np.float32)
    duck = np.zeros(n_total, dtype=np.float32)  # sidechain envelope from kicks

    roots = [55.00, 55.00, 43.65, 49.00, 55.00, 55.00, 65.41, 49.00]  # A1 A1 F1 G1 A1 A1 C2 G1
    penta = [220.00, 261.63, 293.66, 329.63, 392.00, 440.00]          # A3 C4 D4 E4 G4 A4

    def add(dst: np.ndarray, src: np.ndarray, i0: int) -> None:
        n = src.shape[0]
        if i0 >= n_total:
            return
        n = min(n, n_total - i0)
        dst[i0:i0 + n] += src[:n]

    def add_stereo(l_sig, r_sig, i0: int) -> None:
        add(left, l_sig, i0)
        add(right, r_sig, i0)

    def add_kick(t0: float) -> None:
        n = int(0.30 * SR)
        t = np.arange(n, dtype=np.float32) / SR
        phase = 2.0 * np.pi * (40.0 * t + 55.0 * 0.05 * (1.0 - np.exp(-t / 0.05)))
        env = np.exp(-t / 0.105)
        click = rng.standard_normal(n).astype(np.float32) * np.exp(-t / 0.008) * 0.30
        s = np.sin(phase) * env * 0.85 + click
        i0 = int(t0 * SR)
        add(left, s, i0)
        add(right, s, i0)
        duck[i0:i0 + n] += np.exp(-t / 0.17) * 0.75

    def add_snare(t0: float) -> None:
        n = int(0.22 * SR)
        t = np.arange(n, dtype=np.float32) / SR
        noise = rng.standard_normal(n).astype(np.float32)
        low = np.convolve(noise, np.ones(16, np.float32) / 16.0, mode="same")
        s = (noise - low) * np.exp(-t / 0.042) * 0.62 + np.sin(2.0 * np.pi * 185.0 * t) * np.exp(-t / 0.055) * 0.38
        i0 = int(t0 * SR)
        add(left, s * 0.9, i0)
        add(right, s * 0.98, i0)

    def add_hat(t0: float, vol: float = 1.0, pan: float = 0.5) -> None:
        n = int(0.05 * SR)
        t = np.arange(n, dtype=np.float32) / SR
        noise = rng.standard_normal(n).astype(np.float32)
        hp = np.diff(noise, prepend=0.0)
        s = hp * np.exp(-t / 0.011) * 0.36 * vol
        i0 = int(t0 * SR)
        add(left, s * (1.0 - pan), i0)
        add(right, s * pan, i0)

    def add_bass(t0: float, freq: float, dur: float) -> None:
        n = int(dur * SR)
        t = np.arange(n, dtype=np.float32) / SR
        ph = freq * t
        saw = 2.0 * (ph - np.floor(ph)) - 1.0
        ph2 = freq * 1.007 * t
        saw2 = 2.0 * (ph2 - np.floor(ph2)) - 1.0
        body = np.sin(2.0 * np.pi * freq * t)
        x = (saw * 0.42 + saw2 * 0.42 + body * 0.62)
        x = np.tanh(2.3 * x)
        env = np.minimum(1.0, t / 0.008) * np.exp(-np.maximum(0.0, t - (dur - 0.09)) / 0.055)
        i0 = int(t0 * SR)
        seg = min(n, n_total - i0)
        d = duck[i0:i0 + seg]
        s = (x * env * 0.52 * (1.0 - 0.55 * d)).astype(np.float32)
        add(left, s, i0)
        add(right, s * 0.96, i0)

    def add_clang(t0: float, freq: float, pan: float, amp: float = 0.30) -> None:
        n = int(0.52 * SR)
        t = np.arange(n, dtype=np.float32) / SR
        mod_idx = 2.6 * np.exp(-t * 5.0)
        s = np.sin(2.0 * np.pi * freq * t + mod_idx * np.sin(2.0 * np.pi * freq * 1.51 * t))
        s = (s * np.exp(-t / 0.27) * amp).astype(np.float32)
        i0 = int(t0 * SR)
        seg = min(n, n_total - i0)
        d = duck[i0:i0 + seg]
        s = s * (1.0 - 0.45 * d)
        add(left, s * (1.0 - pan), i0)
        add(right, s * pan, i0)

    def add_lead(t0: float, freq: float, dur: float) -> None:
        n = int(dur * SR)
        t = np.arange(n, dtype=np.float32) / SR
        vib = 0.045 * np.sin(2.0 * np.pi * 5.4 * t)
        sq = np.tanh(1.7 * np.sin(2.0 * np.pi * freq * t + vib))
        env = np.minimum(1.0, t / 0.025) * np.exp(-np.maximum(0.0, t - (dur - 0.18)) / 0.15)
        s = (sq * env * 0.15).astype(np.float32)
        i0 = int(t0 * SR)
        add(left, s * 0.92, i0)
        add(right, s * 1.0, i0)

    def add_hiss_swell(t0: float) -> None:
        n = int(1.9 * SR)
        t = np.arange(n, dtype=np.float32) / SR
        noise = rng.standard_normal(n).astype(np.float32)
        low = np.convolve(noise, np.ones(220, np.float32) / 220.0, mode="same")
        s = (noise - low * 0.55) * np.sin(np.pi * t / 1.9) ** 2 * 0.11
        i0 = int(t0 * SR)
        add(left, s * 0.85, i0)
        add(right, s * 1.0, i0)

    # ---- deterministic 16th-note arp gate plan (A/B sections)
    arp_steps = [step for step in range(16 * bars) if rng.random() < 0.40]
    arp_notes = {step: int(rng.integers(0, len(penta))) for step in arp_steps}
    hat_doubles = {step: rng.random() < 0.22 for step in range(16 * bars)}
    hat_vol = {step: float(rng.uniform(0.65, 1.0)) for step in range(16 * bars)}
    bass_oct = {step: rng.random() < 0.18 for step in range(8 * bars)}

    for bar_i in range(bars):
        t_bar = bar_i * bar
        root = roots[bar_i % len(roots)]
        section = "A" if bar_i < 32 else ("B" if bar_i < 64 else ("A2" if bar_i < 80 else ("B2" if bar_i < 112 else "OUT")))

        # kick
        if section == "OUT":
            kicks = [0.0]
        elif section == "A2":
            kicks = [0.0, 1.2, 2.0, 3.0] if bar_i % 4 == 2 else [0.0, 2.0]
        else:
            kicks = [0.0, 2.0] + ([3.5] if bar_i % 4 == 3 else [])
        for beat in kicks:
            add_kick(t_bar + beat * spb)

        # snare
        if section != "OUT":
            add_snare(t_bar + 1.0 * spb)
            add_snare(t_bar + 3.0 * spb)

        # hats (offbeat 8ths; A2 busier; outro none)
        if section != "OUT":
            offbeats = [0.5, 1.5, 2.5, 3.5]
            if section == "A2":
                offbeats = [0.0, 0.5, 1.5, 2.0, 2.5, 3.0, 3.5]
            for i, off in enumerate(offbeats):
                add_hat(t_bar + off * spb, vol=float(rng.uniform(0.7, 1.0)) if i % 3 else 0.6, pan=0.62)

        # bass: 8ths on the root, occasional octave flip
        step8 = bar_i * 8
        for e in range(8):
            t0 = t_bar + e * 0.5 * spb
            if section == "OUT":
                if e == 0:
                    add_bass(t0, root, bar * 0.98)
                continue
            freq = root * (2.0 if bass_oct.get(step8 + e, False) else 1.0)
            add_bass(t0, freq, 0.52 * spb)

        # arp clangs (B sections only): seeded 16ths from the A-minor pentatonic
        if section in ("B", "B2"):
            s16 = bar_i * 16
            for step in range(16):
                note = arp_notes.get(s16 + step)
                if note is None:
                    continue
                add_clang(t_bar + step * 0.25 * spb, penta[note], pan=float(rng.uniform(0.35, 0.65)),
                          amp=float(rng.uniform(0.17, 0.30)))

        # lead motif (B2, bars 84..99): whole-note descends
        if section == "B2" and 84 <= bar_i < 100:
            motif = [440.00, 392.00, 329.63, 293.66]
            add_lead(t_bar, motif[(bar_i - 84) % len(motif)], bar * 0.96)

        # hiss swells at section exits
        if bar_i + 1 in (32, 64, 80, 112):
            add_hiss_swell(t_bar + bar - 1.9)

    # final hit: bar 124 beat 0 (index 124)
    t_final = 124 * bar
    add_kick(t_final)
    n = int(1.5 * SR)
    t = np.arange(n, dtype=np.float32) / SR
    noise = rng.standard_normal(n).astype(np.float32)
    low = np.convolve(noise, np.ones(640, np.float32) / 640.0, mode="same")
    crash = (noise * 0.25 + low * 0.75) * np.exp(-t / 0.62) * 0.6
    add(left, crash, int(t_final * SR))
    add(right, crash, int(t_final * SR))

    # constant industrial room-tone bed
    noise = rng.standard_normal(n_total).astype(np.float32)
    bed = np.convolve(noise, np.ones(90, np.float32) / 90.0, mode="same")
    left *= 1.0
    right *= 1.0
    left += bed * 0.016
    right += bed * 0.020

    # master: soft clip + normalize to -1 dBFS
    left = np.tanh(left * 1.18)
    right = np.tanh(right * 1.18)
    peak = max(float(np.max(np.abs(left))), float(np.max(np.abs(right))), 1e-6)
    gain = 0.891 / peak
    stereo = np.empty(n_total * 2, dtype=np.float32)
    stereo[0::2] = left * gain
    stereo[1::2] = right * gain
    write_wav(MUS / "industry_legend_5min.wav", stereo, stereo=True)


# ---------------------------------------------------------------- manifest

def main() -> int:
    TEX.mkdir(parents=True, exist_ok=True)
    SFX.mkdir(parents=True, exist_ok=True)
    MUS.mkdir(parents=True, exist_ok=True)

    print("RAILGUN PURSUIT 3D — procedural asset pack")
    print("textures (4K, JPEG q88):")
    gen_asphalt()
    gen_sand()
    gen_concrete()
    print("sfx (44.1 kHz 16-bit mono):")
    gen_engine_loop()
    gen_wind_loop()
    gen_shot_ap()
    gen_shot_inc()
    gen_explosion_big()
    gen_explosion_small()
    gen_ricochet()
    gen_pickup_scrap()
    gen_pickup_medkit()
    gen_overheat()
    gen_mine_beep()
    gen_ram_crash()
    gen_workbench()
    print("music:")
    gen_music()

    WRITTEN.sort(key=lambda item: item[0])
    manifest_path = ROOT / "assets" / "manifest.json"
    entries = [{"path": name, "bytes": size} for name, size in WRITTEN]
    manifest_path.write_text(json.dumps(entries, indent=2) + "\n", encoding="utf-8")
    total = sum(size for _, size in WRITTEN)
    print(f"\nmanifest: {manifest_path.relative_to(ROOT)} — {len(entries)} files, {total / 1e6:.1f} MB total")
    print("note: procedural placeholders — NOT hand-authored 4K PBR art / mastered audio.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
