"""Writes crates/app/assets/icon.ico: the app's icon for the executable.

The icon is the one `crates/app/src/icon.rs` draws at run time for the
window and the tray: a red disc with a play triangle. Windows wants a file
at build time, so the same drawing is made here once and committed. If the
drawing changes there, change it here and run this again.

    python scripts/make-icon.py
"""
import os
import struct
import zlib

DISC = (0xFF, 0x00, 0x33)
GLYPH = (0xFF, 0xFF, 0xFF)
SAMPLES = 4
SIZES = [256, 48, 32, 16]


def in_play_triangle(x, y):
    left, right, half_height = -0.28, 0.46, 0.44
    if x < left or x > right:
        return False
    return abs(y) <= half_height * (right - x) / (right - left)


def pixels(size):
    rows = []
    for y in range(size):
        row = bytearray([0])  # each PNG row starts with its filter type
        for x in range(size):
            in_disc = in_glyph = 0
            for sample in range(SAMPLES * SAMPLES):
                px = (x + (sample % SAMPLES + 0.5) / SAMPLES) / size * 2 - 1
                py = (y + (sample // SAMPLES + 0.5) / SAMPLES) / size * 2 - 1
                if px * px + py * py <= 1:
                    in_disc += 1
                    in_glyph += in_play_triangle(px, py)
            share = in_glyph / max(in_disc, 1)
            row += bytes(int(d + (g - d) * share) for d, g in zip(DISC, GLYPH))
            row.append(int(in_disc / (SAMPLES * SAMPLES) * 255))
        rows.append(bytes(row))
    return b"".join(rows)


def png(size):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(pixels(size), 9))
        + chunk(b"IEND", b"")
    )


def ico(images):
    """An .ico holding each size as a PNG, which Windows has read since Vista."""
    directory = struct.pack("<HHH", 0, 1, len(images))
    offset = len(directory) + 16 * len(images)
    entries, bodies = b"", b""
    for size, data in images:
        side = 0 if size == 256 else size  # 0 stands for 256
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        bodies += data
        offset += len(data)
    return directory + entries + bodies


if __name__ == "__main__":
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    target = os.path.join(root, "crates", "app", "assets", "icon.ico")
    with open(target, "wb") as out:
        out.write(ico([(size, png(size)) for size in SIZES]))
    print("wrote", target)
