#!/usr/bin/env python3
"""Regenerate Meetdy tray icons.

Produces 44x44 PNGs (suitable for macOS @2x menu bar, ~22pt) for three states
(idle, recording, transcribing) in two color variants:
  - light glyph on transparent bg (used when system theme is Dark)
  - dark glyph on transparent bg (used when system theme is Light)

Plus the "Colored" variants (meetdy.png / recording.png / transcribing.png)
used on Linux: pink/red/yellow filled circle with the M glyph.

Glyph: a stylized "M" with state indicators:
  - idle:         M only
  - recording:    M + small red filled dot at top-right
  - transcribing: M + three dots underneath
"""
from __future__ import annotations
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "resources"
SIZE = 44  # 22pt @2x for macOS menu bar
PAD = 3

LIGHT = (245, 245, 245, 255)   # for dark theme
DARK = (30, 30, 30, 255)       # for light theme
PINK = (255, 105, 180, 255)    # colored "Idle" (Linux)
RED = (220, 50, 50, 255)
YELLOW = (240, 190, 50, 255)


def _font(size: int) -> ImageFont.FreeTypeFont:
    candidates = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/Library/Fonts/Arial Bold.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ]
    for path in candidates:
        if Path(path).exists():
            try:
                return ImageFont.truetype(path, size)
            except OSError:
                continue
    return ImageFont.load_default()


def _draw_m(img: Image.Image, color: tuple[int, int, int, int]) -> None:
    draw = ImageDraw.Draw(img)
    font = _font(34)
    text = "M"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    x = (SIZE - tw) / 2 - bbox[0]
    y = (SIZE - th) / 2 - bbox[1] - 1
    draw.text((x, y), text, fill=color, font=font)


def _draw_recording_dot(img: Image.Image) -> None:
    draw = ImageDraw.Draw(img)
    r = 6
    cx, cy = SIZE - r - 2, r + 2
    draw.ellipse((cx - r, cy - r, cx + r, cy + r), fill=RED)


def _draw_transcribing_dots(img: Image.Image, color: tuple[int, int, int, int]) -> None:
    draw = ImageDraw.Draw(img)
    r = 2
    y = SIZE - PAD - r
    spacing = 8
    cx = SIZE // 2
    for dx in (-spacing, 0, spacing):
        draw.ellipse((cx + dx - r, y - r, cx + dx + r, y + r), fill=color)


def make(state: str, color: tuple[int, int, int, int], path: Path) -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    _draw_m(img, color)
    if state == "recording":
        _draw_recording_dot(img)
    elif state == "transcribing":
        _draw_transcribing_dots(img, color)
    img.save(path, "PNG")
    print(f"  wrote {path.relative_to(ROOT)}")


def make_colored(state: str, path: Path) -> None:
    """Colored variant used on Linux: filled circle backdrop + white M."""
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    bg = {"idle": PINK, "recording": RED, "transcribing": YELLOW}[state]
    draw.ellipse((1, 1, SIZE - 1, SIZE - 1), fill=bg)
    _draw_m(img, LIGHT)
    img.save(path, "PNG")
    print(f"  wrote {path.relative_to(ROOT)}")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    print("Dark theme (light glyph):")
    make("idle", LIGHT, OUT / "tray_idle.png")
    make("recording", LIGHT, OUT / "tray_recording.png")
    make("transcribing", LIGHT, OUT / "tray_transcribing.png")
    print("Light theme (dark glyph):")
    make("idle", DARK, OUT / "tray_idle_dark.png")
    make("recording", DARK, OUT / "tray_recording_dark.png")
    make("transcribing", DARK, OUT / "tray_transcribing_dark.png")
    print("Colored (Linux):")
    make_colored("idle", OUT / "meetdy.png")
    make_colored("recording", OUT / "recording.png")
    make_colored("transcribing", OUT / "transcribing.png")


if __name__ == "__main__":
    main()
