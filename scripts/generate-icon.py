#!/usr/bin/env python3
"""Generates KeyFlow's app icon as a raw 1024x1024 PNG.

No image libraries required (stdlib zlib + struct only), so this runs
anywhere Python 3 does. It draws an original mark — not a copy of any
existing product's icon: a rounded-square gradient background, a
stylized key (circular bow + tapered shaft + two teeth), and a curved
"flow" swoosh beneath it.

Regenerate with:
    python3 scripts/generate-icon.py apps/desktop/src-tauri/icons/icon-source.png
Then feed that into `cargo tauri icon` to produce the full platform set.
"""
import struct
import sys
import zlib

SIZE = 1024


def lerp(a, b, t):
    return a + (b - a) * t


def bg_color(x, y):
    # Diagonal gradient, deep indigo -> teal, evoking "security" (indigo)
    # blending into "flow" (teal).
    t = (x + y) / (2 * SIZE)
    r = int(lerp(0x2A, 0x14, t))
    g = int(lerp(0x1F, 0xB8, t))
    b = int(lerp(0x6B, 0xA6, t))
    return r, g, b


def rounded_square_mask(x, y, size, radius):
    cx0, cy0 = radius, radius
    cx1, cy1 = size - radius, radius
    cx2, cy2 = radius, size - radius
    cx3, cy3 = size - radius, size - radius
    if x < radius and y < radius:
        return (x - cx0) ** 2 + (y - cy0) ** 2 <= radius ** 2
    if x > size - radius and y < radius:
        return (x - cx1) ** 2 + (y - cy1) ** 2 <= radius ** 2
    if x < radius and y > size - radius:
        return (x - cx2) ** 2 + (y - cy2) ** 2 <= radius ** 2
    if x > size - radius and y > size - radius:
        return (x - cx3) ** 2 + (y - cy3) ** 2 <= radius ** 2
    return True


def dist_to_segment(px, py, x1, y1, x2, y2):
    dx, dy = x2 - x1, y2 - y1
    length_sq = dx * dx + dy * dy
    if length_sq == 0:
        return ((px - x1) ** 2 + (py - y1) ** 2) ** 0.5
    t = max(0.0, min(1.0, ((px - x1) * dx + (py - y1) * dy) / length_sq))
    proj_x, proj_y = x1 + t * dx, y1 + t * dy
    return ((px - proj_x) ** 2 + (py - proj_y) ** 2) ** 0.5


def key_mask(x, y):
    """A simple original key glyph centered in the icon, in a local
    coordinate space of 0..100 mapped onto the full canvas."""
    u = x / SIZE * 100
    v = y / SIZE * 100

    # Bow (the round part of the key) is an off-center ring.
    bow_cx, bow_cy, bow_r, bow_inner = 38, 40, 15, 7
    d_bow = ((u - bow_cx) ** 2 + (v - bow_cy) ** 2) ** 0.5
    in_bow_ring = bow_inner <= d_bow <= bow_r

    # Shaft: a thick diagonal line from the bow out to the lower right.
    shaft_half_width = 6
    d_shaft = dist_to_segment(u, v, bow_cx + 9, bow_cy + 9, 78, 80)
    in_shaft = d_shaft <= shaft_half_width and u >= bow_cx and v >= bow_cy

    # Two teeth near the tip of the shaft, perpendicular notches.
    in_tooth1 = dist_to_segment(u, v, 66, 62, 74, 54) <= 4.5
    in_tooth2 = dist_to_segment(u, v, 74, 70, 82, 62) <= 4.5

    return in_bow_ring or in_shaft or in_tooth1 or in_tooth2


def flow_mask(x, y):
    """A thin curved swoosh beneath the key suggesting motion/"flow"."""
    u = x / SIZE * 100
    v = y / SIZE * 100
    if v < 78 or v > 92:
        return False
    # y = a * sin curve across x, offset downward.
    import math

    curve_y = 85 + 4 * math.sin((u - 10) / 70 * math.pi)
    return abs(v - curve_y) <= 1.6 and 12 <= u <= 88


def build_pixels():
    pixels = bytearray()
    radius = int(SIZE * 0.22)
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            if not rounded_square_mask(x, y, SIZE, radius):
                row += b"\x00\x00\x00\x00"
                continue
            r, g, b = bg_color(x, y)
            if key_mask(x, y):
                row += bytes([0xF5, 0xF7, 0xFA, 0xFF])
            elif flow_mask(x, y):
                row += bytes([0xF5, 0xF7, 0xFA, 0xC8])
            else:
                row += bytes([r, g, b, 0xFF])
        pixels += row
    return bytes(pixels)


def write_png(path, size, rgba_bytes):
    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)

    raw = bytearray()
    stride = size * 4
    for y in range(size):
        raw.append(0)  # no filter
        raw += rgba_bytes[y * stride:(y + 1) * stride]
    idat = zlib.compress(bytes(raw), 9)

    with open(path, "wb") as f:
        f.write(sig)
        f.write(chunk(b"IHDR", ihdr))
        f.write(chunk(b"IDAT", idat))
        f.write(chunk(b"IEND", b""))


if __name__ == "__main__":
    out_path = sys.argv[1] if len(sys.argv) > 1 else "icon-source.png"
    write_png(out_path, SIZE, build_pixels())
    print(f"wrote {out_path}")
