from __future__ import annotations

import binascii
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def chunk(kind: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", binascii.crc32(kind + data) & 0xFFFFFFFF)
    )


def png_rgba(width: int, height: int, pixels: bytes) -> bytes:
    rows = []
    stride = width * 4
    for y in range(height):
        rows.append(b"\x00" + pixels[y * stride : (y + 1) * stride])
    raw = b"".join(rows)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def make_storm_rgba(size: int) -> bytes:
    scale = 4
    width = height = size * scale
    pixels = [(0, 0, 0, 0)] * (width * height)

    def blend(x: int, y: int, color: tuple[int, int, int, int]) -> None:
        if x < 0 or y < 0 or x >= width or y >= height:
            return
        sr, sg, sb, sa = color
        if sa == 0:
            return
        index = y * width + x
        dr, dg, db, da = pixels[index]
        alpha = sa / 255.0
        dest_alpha = da / 255.0
        out_alpha = alpha + dest_alpha * (1.0 - alpha)
        if out_alpha <= 0.0:
            pixels[index] = (0, 0, 0, 0)
            return
        pixels[index] = (
            round((sr * alpha + dr * dest_alpha * (1.0 - alpha)) / out_alpha),
            round((sg * alpha + dg * dest_alpha * (1.0 - alpha)) / out_alpha),
            round((sb * alpha + db * dest_alpha * (1.0 - alpha)) / out_alpha),
            round(out_alpha * 255.0),
        )

    def circle(cx: float, cy: float, radius: float, color: tuple[int, int, int, int]) -> None:
        cx *= scale
        cy *= scale
        radius *= scale
        rr = radius * radius
        for y in range(max(0, int(cy - radius - 1)), min(height, int(cy + radius + 2))):
            for x in range(max(0, int(cx - radius - 1)), min(width, int(cx + radius + 2))):
                dx = x + 0.5 - cx
                dy = y + 0.5 - cy
                if dx * dx + dy * dy <= rr:
                    blend(x, y, color)

    def rectangle(
        left: float,
        top: float,
        right: float,
        bottom: float,
        color: tuple[int, int, int, int],
    ) -> None:
        for y in range(max(0, int(top * scale)), min(height, int(bottom * scale + 1))):
            for x in range(max(0, int(left * scale)), min(width, int(right * scale + 1))):
                blend(x, y, color)

    def polygon(points: list[tuple[float, float]], color: tuple[int, int, int, int]) -> None:
        pts = [(x * scale, y * scale) for x, y in points]
        min_x = max(0, int(min(x for x, _ in pts)))
        max_x = min(width - 1, int(max(x for x, _ in pts) + 1))
        min_y = max(0, int(min(y for _, y in pts)))
        max_y = min(height - 1, int(max(y for _, y in pts) + 1))
        for y in range(min_y, max_y + 1):
            for x in range(min_x, max_x + 1):
                inside = False
                j = len(pts) - 1
                for i, (xi, yi) in enumerate(pts):
                    xj, yj = pts[j]
                    intersects = ((yi > y + 0.5) != (yj > y + 0.5)) and (
                        x + 0.5
                        < (xj - xi) * (y + 0.5 - yi) / ((yj - yi) or 1e-9) + xi
                    )
                    if intersects:
                        inside = not inside
                    j = i
                if inside:
                    blend(x, y, color)

    s = size / 32.0
    for cx, cy, radius in [(9, 14, 9), (17, 10, 10), (24, 15, 9)]:
        circle(cx * s, cy * s, radius * s, (31, 200, 255, 60))
    rectangle(4 * s, 12 * s, 29 * s, 22 * s, (31, 200, 255, 50))

    for cx, cy, radius in [(9, 14, 7.2), (17, 10, 8.5), (24, 15, 7.2)]:
        circle(cx * s, cy * s, (radius + 1.0) * s, (43, 151, 230, 245))
        circle(cx * s, cy * s, radius * s, (11, 25, 52, 255))
    rectangle(5 * s, 13 * s, 28 * s, 21 * s, (11, 25, 52, 255))

    bolt = [(15, 15), (23, 15), (19, 21), (23, 21), (11, 31), (15, 23), (11, 23)]
    for dx, dy, alpha in [(-1.4, 0, 60), (1.4, 0, 60), (0, 1.4, 70)]:
        polygon([((x + dx) * s, (y + dy) * s) for x, y in bolt], (58, 213, 255, alpha))
    polygon([(x * s, y * s) for x, y in bolt], (255, 231, 100, 255))
    polygon(
        [
            (17 * s, 16 * s),
            (21 * s, 16 * s),
            (17 * s, 21 * s),
            (19 * s, 21 * s),
            (14 * s, 26 * s),
        ],
        (255, 255, 255, 130),
    )

    output = bytearray()
    for y in range(size):
        for x in range(size):
            samples = [
                pixels[(y * scale + sy) * width + (x * scale + sx)]
                for sy in range(scale)
                for sx in range(scale)
            ]
            output.extend(
                round(sum(sample[channel] for sample in samples) / len(samples))
                for channel in range(4)
            )
    return bytes(output)


def write_ico(path: str) -> None:
    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    images = [png_rgba(size, size, make_storm_rgba(size)) for size in sizes]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = []
    for size, image in zip(sizes, images):
        entries.append(
            struct.pack(
                "<BBBBHHII",
                0 if size == 256 else size,
                0 if size == 256 else size,
                0,
                0,
                1,
                32,
                len(image),
                offset,
            )
        )
        offset += len(image)
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(header + b"".join(entries) + b"".join(images))


if __name__ == "__main__":
    write_ico("assets/respanso.ico")
