#!/usr/bin/env python3
"""Generates Android adaptive-icon assets from the existing KeyFlow brand
icon (apps/desktop/src-tauri/icons/icon-source.png), so the Android app
uses the same visual identity as desktop/browser-extension rather than a
new, separate icon.

Adaptive icons (API 26+, which is this app's minSdk, so no legacy square
launcher icon is needed at all) use two 108dp x 108dp layers; only the
center ~66dp is guaranteed visible under every OEM launcher's mask
(circle, squircle, rounded square), so the source logo — a full-bleed
1024x1024 rounded square with no built-in margin — is scaled down and
centered with transparent padding before export, rather than used at
100% (which would get clipped by circular masks).

Run manually when the source brand icon changes:
    python3 scripts/generate-android-icons.py
"""
from pathlib import Path

from PIL import Image

REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE = REPO_ROOT / "apps/desktop/src-tauri/icons/icon-source.png"
RES_DIR = REPO_ROOT / "apps/android/app/src/main/res"

# dp -> px scale factor per density bucket, for a 108dp adaptive-icon canvas.
DENSITIES = {
    "mdpi": 1.0,
    "hdpi": 1.5,
    "xhdpi": 2.0,
    "xxhdpi": 3.0,
    "xxxhdpi": 4.0,
}
CANVAS_DP = 108
# Content occupies ~62% of the canvas, comfortably inside the ~66dp
# guaranteed-visible safe zone under a circular mask.
CONTENT_SCALE = 0.62


def build_foreground(size_px: int) -> Image.Image:
    canvas = Image.new("RGBA", (size_px, size_px), (0, 0, 0, 0))
    logo_size = round(size_px * CONTENT_SCALE)
    logo = Image.open(SOURCE).convert("RGBA").resize((logo_size, logo_size), Image.LANCZOS)
    offset = (size_px - logo_size) // 2
    canvas.paste(logo, (offset, offset), logo)
    return canvas


def build_monochrome(size_px: int) -> Image.Image:
    """Android 13+ themed-icon layer: a single-alpha silhouette (the OS
    applies its own tint color at runtime), derived from the same logo's
    luminance so the glyph shape survives even when the OS recolors it."""
    logo_size = round(size_px * CONTENT_SCALE)
    logo = Image.open(SOURCE).convert("RGBA").resize((logo_size, logo_size), Image.LANCZOS)
    gray = logo.convert("L")
    # The glyph is drawn in a light color over a darker gradient — treat
    # brighter pixels as opaque glyph, darker background as transparent.
    alpha = gray.point(lambda p: 255 if p > 150 else 0)
    silhouette = Image.new("RGBA", (logo_size, logo_size), (255, 255, 255, 0))
    silhouette.putalpha(alpha)
    canvas = Image.new("RGBA", (size_px, size_px), (0, 0, 0, 0))
    offset = (size_px - logo_size) // 2
    canvas.paste(silhouette, (offset, offset), silhouette)
    return canvas


def main() -> None:
    if not SOURCE.exists():
        raise SystemExit(f"source icon not found: {SOURCE}")

    for density, scale in DENSITIES.items():
        size_px = round(CANVAS_DP * scale)
        out_dir = RES_DIR / f"mipmap-{density}"
        out_dir.mkdir(parents=True, exist_ok=True)
        build_foreground(size_px).save(out_dir / "ic_launcher_foreground.png")

        mono_dir = RES_DIR / f"drawable-{density}"
        mono_dir.mkdir(parents=True, exist_ok=True)
        build_monochrome(size_px).save(mono_dir / "ic_launcher_monochrome.png")
        print(f"{density}: {size_px}x{size_px}")

    print("Done. Adaptive icon XML (mipmap-anydpi-v26) is checked in separately and references these files by name.")


if __name__ == "__main__":
    main()
