#!/usr/bin/env python3
"""RAILGUN PURSUIT 3D — launcher icon generator (Module 6).

Geometric, deterministic, procedural. Honest placeholder: a stylized war-rig
front (railgun coils, slot windshield, bullbar, amber headlights) drawn with
primitives only — no text, no traced artwork. Replace with artist masters for
a commercial release; see android_res/README.md.

Usage: python3 tools/gen_icon.py
Writes android_res/mipmap-<density>/ic_launcher.png for the five densities
cargo-quad-apk/aapt expects.
"""

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

S = 512
ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "android_res"


def vertical_gradient(size: int, top, bottom) -> np.ndarray:
    t = np.linspace(0.0, 1.0, size, dtype=np.float32)[:, None, None]
    top = np.array(top, dtype=np.float32)[None, None, :]
    bottom = np.array(bottom, dtype=np.float32)[None, None, :]
    row = top * (1.0 - t) + bottom * t  # (size, 1, 3)
    return np.broadcast_to(row, (size, size, 3)).copy()


def add_radial_glow(img: np.ndarray, cx: float, cy: float, radius: float, color, strength: float) -> None:
    yy, xx = np.mgrid[0:img.shape[0], 0:img.shape[1]].astype(np.float32)
    dist2 = (xx - cx) ** 2 + (yy - cy) ** 2
    glow = np.exp(-dist2 / (radius * radius * 0.35)) * strength
    color = np.array(color, dtype=np.float32)
    img[:] = np.clip(img + glow[..., None] * color, 0, 255)


def add_vignette(img: np.ndarray, strength: float = 0.42) -> None:
    yy, xx = np.mgrid[0:img.shape[0], 0:img.shape[1]].astype(np.float32)
    d = np.sqrt(((xx - S / 2) / (S * 0.72)) ** 2 + ((yy - S / 2) / (S * 0.72)) ** 2)
    img[:] = np.clip(img * (1.0 - strength * np.clip(d - 0.55, 0.0, 1.0))[..., None], 0, 255)


def build() -> Image.Image:
    rng = np.random.default_rng(7)
    img = vertical_gradient(S, (52, 46, 38), (16, 14, 12))
    img += rng.normal(0.0, 3.0, img.shape)  # film grain
    img = np.clip(img, 0, 255)

    # glows (drawn into the pixel buffer for smooth falloff)
    add_radial_glow(img, 256, 118, 260, (255, 138, 30), 0.34)  # railgun ambience
    add_radial_glow(img, 178, 338, 90, (255, 176, 64), 0.55)   # left headlight
    add_radial_glow(img, 334, 338, 90, (255, 176, 64), 0.55)   # right headlight

    im = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8), "RGB")
    d = ImageDraw.Draw(im, "RGBA")

    # ---- railgun barrel + coils ----
    d.rounded_rectangle([96, 96, 416, 132], 6, fill=(34, 36, 42, 255))
    d.rectangle([96, 100, 416, 103], fill=(64, 67, 76, 255))
    for cx in (172, 240, 308):
        d.rounded_rectangle([cx - 14, 84, cx + 14, 146], 5, fill=(58, 44, 26, 255))
        d.rounded_rectangle([cx - 11, 88, cx + 11, 142], 4, fill=(255, 150, 40, 255))
        d.rectangle([cx - 11, 88, cx + 11, 96], fill=(255, 205, 110, 255))

    # ---- cab silhouette (armored trapezoid) ----
    d.polygon([(126, 208), (386, 208), (404, 362), (108, 362)], fill=(59, 65, 72, 255))
    d.polygon([(126, 208), (386, 208), (395, 236), (117, 236)], fill=(76, 83, 92, 255))
    d.line([(126, 208), (386, 208)], fill=(120, 128, 140, 255), width=2)

    # bolt plates
    for (bx, by) in [(136, 252), (364, 252), (130, 330), (370, 330), (256, 234)]:
        d.rectangle([bx - 6, by - 6, bx + 6, by + 6], fill=(46, 50, 56, 255))
        d.rectangle([bx - 6, by - 6, bx + 6, by - 4], fill=(96, 103, 112, 255))

    # windshield slit + sheen
    d.polygon([(168, 240), (344, 240), (352, 266), (160, 266)], fill=(13, 17, 21, 255))
    d.polygon([(176, 244), (250, 244), (236, 258), (170, 258)], fill=(52, 62, 72, 180))

    # headlights (behind the bullbar)
    for cx in (178, 334):
        d.ellipse([cx - 15, 322, cx + 15, 352], fill=(255, 176, 64, 255))
        d.ellipse([cx - 8, 329, cx + 8, 345], fill=(255, 228, 150, 255))

    # ---- bullbar (rust) over the lights ----
    for cx in (160, 352):
        d.rounded_rectangle([cx - 13, 252, cx + 13, 396], 6, fill=(124, 79, 36, 255))
        d.rectangle([cx - 13, 252, cx - 9, 396], fill=(165, 110, 55, 255))
    d.rounded_rectangle([138, 296, 374, 320], 6, fill=(124, 79, 36, 255))
    d.rectangle([138, 296, 374, 300], fill=(165, 110, 55, 255))

    # bumper
    d.rectangle([96, 362, 416, 388], fill=(30, 32, 37, 255))
    d.rectangle([96, 362, 416, 365], fill=(58, 61, 68, 255))

    # ---- hazard band along the bottom ----
    band_y, band_h, stripe = 452, 22, 26
    d.rectangle([0, band_y, S, band_y + band_h], fill=(16, 14, 12, 255))
    for x in range(-band_h, S + band_h, stripe * 2):
        d.polygon([(x, band_y + band_h), (x + band_h, band_y), (x + band_h + stripe, band_y), (x + stripe, band_y + band_h)],
                  fill=(233, 138, 29, 255))

    img = np.asarray(im).astype(np.float32)
    add_vignette(img)
    return Image.fromarray(np.clip(img, 0, 255).astype(np.uint8), "RGB")


def main() -> int:
    master = build()
    densities = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
    for name, size in densities.items():
        out_dir = RES / f"mipmap-{name}"
        out_dir.mkdir(parents=True, exist_ok=True)
        out_path = out_dir / "ic_launcher.png"
        master.resize((size, size), Image.LANCZOS).save(out_path, "PNG")
        print(f"  icon {name:7s} {size:3d}px  {out_path.stat().st_size:6d} B")
    print("done — procedural placeholder icon (artist masters wanted for release)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
